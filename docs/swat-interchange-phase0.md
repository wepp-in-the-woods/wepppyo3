# SWAT Interchange Phase 0 Discovery
> Fixture catalog and header scan for SWAT+ outputs.

## Fixture
- run_output_dir: `/wc1/runs/pe/pertinent-conventioneer/swat/outputs/run_20260203T042219Z`
- manifest: `/wc1/runs/pe/pertinent-conventioneer/swat/outputs/run_20260203T042219Z/files_out.out`
- entries: 56

## Manifest Summary

| Category | File | Size (MB) | Units Line? | Notes |
| --- | --- | --- | --- | --- |
| CHK | checker.out | 0.00 | yes | manual nonstandard |
| HRU | hru_wb_day.txt | 7.04 | yes |  |
| HRU | hru_wb_mon.txt | 0.23 | yes |  |
| HRU | hru_wb_yr.txt | 0.02 | yes |  |
| HRU | hru_wb_aa.txt | 0.00 | yes |  |
| HRU | hru_ncycle_aa.txt | 0.00 | yes |  |
| HRU | hru_nb_aa.txt | 0.00 | yes |  |
| HRU | hru_soilcarb_aa.txt | 0.00 | yes |  |
| HRU | hru_rescarb_aa.txt | 0.00 | yes |  |
| HRU | hru_plcarb_aa.txt | 0.00 | yes |  |
| HRU | hru_scf_aa.txt | 0.00 | yes |  |
| BASIN | basin_carbon_all.txt | 0.00 | yes |  |
| HRU | hru_nut_carb_gl_aa.txt | 0.00 | yes |  |
| HRU | hru_ls_aa.txt | 0.00 | yes |  |
| HRU | hru_pw_aa.txt | 0.00 | yes |  |
| ROUTING_UNIT | lsunit_wb_aa.txt | 0.00 | yes |  |
| ROUTING_UNIT | lsunit_nb_aa.txt | 0.00 | yes |  |
| ROUTING_UNIT | lsunit_ls_aa.txt | 0.00 | yes |  |
| ROUTING_UNIT | lsunit_pw_aa.txt | 0.00 | yes |  |
| BASIN | basin_wb_day.txt | 6.47 | yes |  |
| BASIN | basin_wb_yr.txt | 0.02 | yes |  |
| BASIN | basin_wb_aa.txt | 0.00 | yes |  |
| BASIN | basin_nb_aa.txt | 0.00 | yes |  |
| BASIN | basin_ls_aa.txt | 0.00 | yes |  |
| BASIN | basin_pw_aa.txt | 0.00 | yes |  |
| SWAT-DEG_CHANNEL | channel_sd_day.txt | 4427.18 | yes |  |
| SWAT-DEG_CHANNEL | channel_sd_aa.txt | 0.37 | yes |  |
| SWAT-DEG_CHANNEL_MORPH | channel_sdmorph_day.txt | 2105.73 | yes |  |
| SWAT-DEG_CHANNEL_MORPH | channel_sdmorph_aa.txt | 0.18 | yes |  |
| SWAT_DEG_CHAN_BUD | sd_chanbud_day.txt | 2387.12 | yes |  |
| SWAT_DEG_CHAN_BUD | sd_chanbud_aa.txt | 0.20 | yes |  |
| DTBL | lu_change_out.txt | 0.00 | no | manual nonstandard |
| HYDOUT | hydout_day.txt | 6426.70 | yes |  |
| HYDOUT | hydout_mon.txt | 211.15 | yes |  |
| HYDOUT | hydout_yr.txt | 17.60 | yes |  |
| HYDIN | hydin_day.txt | 6503.94 | yes |  |
| HYDIN | hydin_mon.txt | 213.69 | yes |  |
| HYDIN | hydin_yr.txt | 17.81 | yes |  |
| DEPO | deposition_day.txt | 0.00 | yes |  |
| DEPO | deposition_mon.txt | 0.00 | yes |  |
| DEPO | deposition_yr.txt | 0.00 | yes |  |
| RES_WET | wetland_aa.txt | 0.00 | yes |  |
| HRU_ORGC | hru_orgc.txt | 0.00 | yes |  |
| BASIN_AQUIFER | basin_aqu_aa.txt | 0.00 | yes |  |
| BASIN_RESERVOIR | basin_res_aa.txt | 0.00 | yes |  |
| RECALL_AA | recall_aa.txt | 0.34 | yes |  |
| BASIN_CHANNEL | basin_cha_day.txt | 0.00 | yes |  |
| BASIN_CHANNEL | basin_cha_mon.txt | 0.00 | yes |  |
| BASIN_CHANNEL | basin_cha_yr.txt | 0.00 | yes |  |
| BASIN_CHANNEL | basin_cha_aa.txt | 0.00 | yes |  |
| BASIN_SWAT_DEG_CHANNEL | basin_sd_cha_aa.txt | 0.00 | yes |  |
| BASIN_SWAT_DEG_CHAN_MORPH | basin_sd_chamorph_aa.txt | 0.00 | yes |  |
| BASIN_SWAT_DEG_CHAN_BUD | basin_sd_chanbud_aa.txt | 0.00 | yes |  |
| BASIN_RECALL_AA | basin_psc_aa.txt | 0.00 | yes |  |
| ROUTING_UNITS | ru_day.txt | 4.37 | yes |  |
| ROUTING_UNITS | ru_aa.txt | 0.00 | yes |  |

## Files Not In Manifest (run dir)
The following files exist in the fixture output directory but are not listed in `files_out.out`, so they are not part of the interchange conversion unless explicitly added:

- area_calc.out
- basin_totc.txt
- co2.out
- cs_aqu.ini
- cs_channel.ini
- cs_hru.ini
- diagnostics.out
- erosion.out
- fort.8000
- fort.8001
- hru_totc.txt
- index.json
- recall_db.rec
- simulation.out
- success.fin

## Nonstandard Candidates
Files flagged as missing an obvious units line (heuristic) or manually marked. Full header samples are in `docs/swat-interchange-phase0.json`.

- `checker.out` (CHK) [manual]
- `lu_change_out.txt` (DTBL) [manual]

## Registry Seed
Seed overrides for nonstandard files live in `docs/swat-interchange-registry-seed.json`. This file records skip/header/unit indices plus initial type hints for Phase 1.

## Header Samples (nonstandard)

### `checker.out`
```text
 Osu                       SWAT+ 2026-02-02        MODULAR Rev 2026.61.0.2.61-18-g34c93dc                 
                                        zmx       usle_k       sumfc       sumul      usle_p     usle_ls        esco        epco     cn3_swf       perco     latq_co   tiledrain 
 sname           hydgrp                 (mm)                    (mm)        (mm)                                                                                     0=notile;1=tile;
```

### `lu_change_out.txt`
```text
 Osu                       SWAT+ 2026-02-02        MODULAR Rev 2026.61.0.2.61-18-g34c93dc                 
          hru       year         mon         day     operation   lu_before         lu_after

```
