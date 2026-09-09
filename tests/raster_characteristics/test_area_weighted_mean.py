"""Exercise the deployed Python/native boundary with actual GeoTIFFs."""
import numpy as np
import pytest
import rasterio
from rasterio.transform import from_origin, Affine
from wepppyo3.raster_characteristics import identify_area_weighted_mean_single_raster_key as mean


def raster(path, values, *, nodata=None, crs='EPSG:32610', transform=None, mask=None):
    data = np.array(values, dtype='float64', ndmin=2)
    with rasterio.open(path, 'w', driver='GTiff', width=data.shape[1], height=data.shape[0], count=1,
                       dtype='float64', crs=crs, transform=transform or from_origin(500000, 5000000, 30, 30), nodata=nodata) as dst:
        dst.write(data, 1)
        if mask is not None:
            dst.write_mask(np.array(mask, dtype='uint8', ndmin=2))
    return str(path)


def test_partial_and_all_missing(tmp_path):
    k = raster(tmp_path/'k.tif', [[11]*5 + [21, 24, 0]])
    p = raster(tmp_path/'p.tif', [[.0001]*4 + [-9999, -9999, 9, 9]], nodata=-9999)
    result = mean(k, p, ignore_keys={0}, default_value=.05)
    assert list(result) == ['11', '21']
    assert result['11'] == dict(mean=pytest.approx(.01008), valid_cell_count=4, missing_cell_count=1, total_cell_count=5)
    assert result['21']['mean'] == .05
    with pytest.raises(ValueError, match='11: 1/5 missing'):
        mean(k, p, ignore_keys={0})


@pytest.mark.parametrize('default', [0, -3, .5])
@pytest.mark.parametrize('nodata', [None, -9999, float('nan')])
def test_generic_masks_nonfinite_and_defaults(tmp_path, default, nodata):
    k = raster(tmp_path/'k.tif', [[11]*7])
    p = raster(tmp_path/'p.tif', [[-2, 0, float('nan'), float('inf'), -float('inf'), 5, nodata if nodata is not None else float('nan')]], nodata=nodata, mask=[[255]*5+[0,255]])
    result = mean(k, p, default_value=default)['11']
    assert result['mean'] == pytest.approx((-2+5*default)/7)
    assert result['valid_cell_count'] == 2
    assert result['missing_cell_count'] == 5


def test_key_masks_nodata_channels_empty(tmp_path):
    k = raster(tmp_path/'k.tif', [[11,12,14,-9999]], nodata=-9999, mask=[[0,255,255,255]])
    p = raster(tmp_path/'p.tif', [[1,2,3,4]])
    assert list(mean(k,p)) == ['12']
    assert mean(k,p,ignore_keys={12}) == {}
    assert list(mean(k,p,ignore_channels=False)) == ['12','14']


@pytest.mark.parametrize('case', ['shape','crs','geographic','no_crs','affine','rotation','singular','band','band_zero','default','fractional_key','missing_file'])
def test_invalid_contracts(tmp_path, case):
    kw={}
    if case=='crs': kw['crs']='EPSG:32611'
    if case=='no_crs': kw['crs']=None
    if case=='geographic': kw['crs']='EPSG:4326'
    if case=='affine': kw['transform']=from_origin(500001,5000000,30,30)
    if case=='rotation': kw['transform']=Affine(30,1,500000,0,-30,5000000)
    if case=='singular': kw['transform']=Affine(30,30,500000,30,30,5000000)
    k=raster(tmp_path/'k.tif', [[11.5 if case=='fractional_key' else 11,11]])
    p=raster(tmp_path/'p.tif', [[1] if case=='shape' else [1,2]], **kw)
    kwargs={}
    if case=='band': kwargs['band_indx']=2
    if case=='band_zero': kwargs['band_indx']=0
    if case=='default': kwargs['default_value']=float('inf')
    if case=='missing_file': p=str(tmp_path/'absent.tif')
    with pytest.raises((ValueError,OSError)):
        mean(k,p,**kwargs)


def test_small_high_value_fraction_and_extremes(tmp_path):
    k=raster(tmp_path/'k.tif', [[11]*100])
    p=raster(tmp_path/'p.tif', [[.0001]*99+[.5]])
    assert mean(k,p)['11']['mean'] == pytest.approx(.005099, rel=1e-12)
    p=raster(tmp_path/'p.tif', [[np.finfo(float).max]*100])
    assert mean(k,p)['11']['mean'] == np.finfo(float).max


def test_signed_cancellation_is_stable(tmp_path):
    k=raster(tmp_path/'k.tif', [[11]*3])
    p=raster(tmp_path/'p.tif', [[1e16,1,-1e16]])
    assert mean(k,p)['11']['mean'] == pytest.approx(1/3, rel=1e-12)
