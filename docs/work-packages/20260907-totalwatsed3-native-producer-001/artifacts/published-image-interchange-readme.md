# Interchange Documentation

_Interchange Version: 1.2 (manifest missing)_

## Hillslope Products

### `H.element.parquet`

Daily hillslope element hydrology and sediment metrics.

| Column | Type | Units | Description |
| --- | --- | --- | --- |
| wepp_id | int32 |  |  |
| ofe_id | int16 |  |  |
| year | int16 |  |  |
| julian | int16 |  |  |
| month | int8 |  |  |
| day_of_month | int8 |  |  |
| water_year | int16 |  |  |
| OFE | int16 |  |  |
| Precip | double | mm |  |
| Runoff | double | mm |  |
| EffInt | double | mm/h | Effective rainfall intensity |
| PeakRO | double | mm/h | Peak runoff rate |
| EffDur | double | h |  |
| Enrich | double |  | Sediment enrichment ratio |
| Keff | double | mm/h | Effective hydraulic conductivity |
| Sm | double | mm |  |
| LeafArea | double |  | Leaf area index |
| CanHgt | double | m | Canopy height |
| Cancov | double | % | Canopy cover |
| IntCov | double | % | Interrill cover |
| RilCov | double | % | Rill cover |
| LivBio | double | kg/m^2 |  |
| DeadBio | double | kg/m^2 |  |
| Ki | double | kg s/m^4 | Interrill erodibility |
| Kr | double | s/m | Rill erodibility |
| Tcrit | double |  |  |
| RilWid | double | m |  |
| SedLeave | double | kg/m |  |
| QRain | double | mm |  |
| QSnow | double | mm |  |

Preview:

wepp_id | ofe_id | year | julian | month | day_of_month | water_year | OFE | Precip | Runoff | EffInt | PeakRO | EffDur | Enrich | Keff | Sm | LeafArea | CanHgt | Cancov | IntCov | RilCov | LivBio | DeadBio | Ki | Kr | Tcrit | RilWid | SedLeave | QRain | QSnow
--- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | ---
 |  |  |  |  |  |  |  | mm | mm | mm/h | mm/h | h |  | mm/h | mm |  | m | % | % | % | kg/m^2 | kg/m^2 | kg s/m^4 | s/m |  | m | kg/m | mm | mm
1 | 1 | 1980 | 1 | 1 | 1 | 1980 | 1 | 0 | 0 | 0 | 0 | 0 | 0 | 60 | 303.09 | 11.875 | 7.789 | 90 | 99.9 | 99.9 | 0.164 | 1.382 | 0.012 | 0.01 | 4 | 0.15 | 0 |  |
1 | 1 | 1980 | 15 | 1 | 15 | 1980 | 1 | 0 | 0 | 0 | 0 | 0 | 0 | 60 | 426.668 | 11.875 | 7.789 | 90 | 99.9 | 99.9 | 0.164 | 1.382 | 0.012 | 0.01 | 4 | 0.15 | 0 |  |
1 | 1 | 1980 | 32 | 2 | 1 | 1980 | 1 | 0 | 0 | 0 | 0 | 0 | 0 | 60 | 405.993 | 11.875 | 7.789 | 90 | 99.9 | 99.9 | 0.164 | 1.382 | 0.012 | 0.01 | 4 | 0.15 | 0 |  |

### `H.wat.parquet`

Hillslope water balance per OFE; aligns with wat.out content.

| Column | Type | Units | Description |
| --- | --- | --- | --- |
| wepp_id | int32 |  |  |
| ofe_id | int16 |  |  |
| year | int16 |  |  |
| sim_day_index | int32 |  | 1-indexed simulation day |
| julian | int16 |  |  |
| month | int8 |  |  |
| day_of_month | int8 |  |  |
| water_year | int16 |  |  |
| OFE | int16 |  |  |
| P | double | mm | Precipitation |
| RM | double | mm | Rainfall+Irrigation+Snowmelt |
| Q | double | mm | Daily runoff over eff length |
| Ep | double | mm | Plant transpiration |
| Es | double | mm | Soil evaporation |
| Er | double | mm | Residue evaporation |
| Dp | double | mm | Deep percolation |
| UpStrmQ | double | mm | Runon added to OFE |
| SubRIn | double | mm | Subsurface runon added to OFE |
| latqcc | double | mm | Lateral subsurface flow |
| Total-Soil Water | double | mm | Unfrozen water in soil profile |
| frozwt | double | mm | Frozen water in soil profile |
| Snow-Water | double | mm | Water in surface snow |
| QOFE | double | mm | Daily runoff scaled to single OFE |
| Tile | double | mm | Tile drainage |
| Irr | double | mm | Irrigation |
| Area | double | m^2 | Area that depths apply over |
| SoilWaterTotal | double | mm | Full-profile soil water depth (watcon + frozwt), optional producer-authoritative term |
| ProfileDepth | double | mm | Full soil profile depth (solthk(nsl)), optional producer-authoritative term |
| ProfilePorosityCap | double | mm | Full-profile porosity storage capacity (sum(por * dg)), optional producer-authoritative term |
| ProfileFCStore | double | mm | Full-profile field-capacity storage (sum(thetfc * dg)), optional producer-authoritative term |
| ProfileWPStore | double | mm | Full-profile wilting-point storage (sum(thetdr * dg)), optional producer-authoritative term |
| InterceptionStorage | double | mm | Plant/residue interception carryover storage (pintlv + resint), optional producer-authoritative term |

Preview:

wepp_id | ofe_id | year | sim_day_index | julian | month | day_of_month | water_year | OFE | P | RM | Q | Ep | Es | Er | Dp | UpStrmQ | SubRIn | latqcc | Total-Soil Water | frozwt | Snow-Water | QOFE | Tile | Irr | Area | SoilWaterTotal | ProfileDepth | ProfilePorosityCap | ProfileFCStore | ProfileWPStore | InterceptionStorage
--- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | ---
 |  |  |  |  |  |  |  |  | mm | mm | mm | mm | mm | mm | mm | mm | mm | mm | mm | mm | mm | mm | mm | mm | m^2 | mm | mm | mm | mm | mm | mm
1 | 1 | 1980 | 1 | 1 | 1 | 1 | 1980 | 1 | 0 | 0 | 0 | 0.84 | 0 | 0 | 0.02 | 0 | 0 | 0.19 | 303.09 | 0 | 0 | 0 | 0 | 0 | 84613.9 |  |  |  |  |  |
1 | 1 | 1980 | 2 | 2 | 1 | 2 | 1980 | 1 | 0 | 0 | 0 | 1.24 | 0.01 | 0 | 0.02 | 0 | 0 | 0.33 | 301.5 | 0 | 0 | 0 | 0 | 0 | 84613.9 |  |  |  |  |  |
1 | 1 | 1980 | 3 | 3 | 1 | 3 | 1980 | 1 | 0 | 0 | 0 | 1.32 | 0.01 | 0 | 0.02 | 0 | 0 | 0.49 | 299.67 | 0 | 0 | 0 | 0 | 0 | 84613.9 |  |  |  |  |  |

### `H.pass.parquet`

Event/subevent sediment and runoff delivery by hillslope (PASS).

| Column | Type | Units | Description |
| --- | --- | --- | --- |
| wepp_id | int32 |  |  |
| event | string |  | Record type: EVENT, SUBEVENT, NO EVENT |
| year | int16 |  |  |
| sim_day_index | int32 |  | 1-indexed simulation day since start year |
| julian | int16 |  |  |
| month | int8 |  |  |
| day_of_month | int8 |  |  |
| water_year | int16 |  |  |
| dur | double | s | Storm duration |
| tcs | double | h | Overland flow time of concentration |
| oalpha | double | unitless | Overland flow alpha parameter |
| runoff | double | m | Runoff depth |
| runvol | double | m^3 | Runoff volume |
| sbrunf | double | m | Subsurface runoff depth |
| sbrunv | double | m^3 | Subsurface runoff volume |
| drainq | double | m/day | Drainage flux |
| drrunv | double | m^3 | Tile Drainage volume |
| peakro | double | m^3/s | Peak runoff rate |
| tdet | double | kg | Total detachment |
| tdep | double | kg | Total deposition |
| sedcon_1 | double | kg/m^3 | Sediment concentration 1 |
| sedcon_2 | double | kg/m^3 | Sediment concentration 2 |
| sedcon_3 | double | kg/m^3 | Sediment concentration 3 |
| sedcon_4 | double | kg/m^3 | Sediment concentration 4 |
| sedcon_5 | double | kg/m^3 | Sediment concentration 5 |
| clot | double | m^3/s | Friction flow 1 |
| slot | double | % | % of exiting sediment in the silt size class |
| saot | double | % | % of exiting sediment in the small aggregate size class |
| laot | double | % | % of exiting sediment in the large aggregate size class |
| sdot | double | % | % of exiting sediment in the sand size class |
| gwbfv | double |  | Groundwater baseflow |
| gwdsv | double |  | Groundwater deep seepage |

Preview:

wepp_id | event | year | sim_day_index | julian | month | day_of_month | water_year | dur | tcs | oalpha | runoff | runvol | sbrunf | sbrunv | drainq | drrunv | peakro | tdet | tdep | sedcon_1 | sedcon_2 | sedcon_3 | sedcon_4 | sedcon_5 | clot | slot | saot | laot | sdot | gwbfv | gwdsv
--- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | ---
 |  |  |  |  |  |  |  | s | h | unitless | m | m^3 | m | m^3 | m/day | m^3 | m^3/s | kg | kg | kg/m^3 | kg/m^3 | kg/m^3 | kg/m^3 | kg/m^3 | m^3/s | % | % | % | % |  |
1 | SUBEVENT | 1980 | 1 | 1 | 1 | 1 | 1980 | 0 | 0 | 0 | 0 | 0 | 0.00019226 | 16.268 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0.062969 | 0
1 | SUBEVENT | 1980 | 2 | 2 | 1 | 2 | 1980 | 0 | 0 | 0 | 0 | 0 | 0.00032752 | 27.713 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0.12342 | 0
1 | SUBEVENT | 1980 | 3 | 3 | 1 | 3 | 1980 | 0 | 0 | 0 | 0 | 0 | 0.00049379 | 41.782 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0.18145 | 0

### `H.soil.parquet`

Daily soil state variables per OFE from soil.dat.

| Column | Type | Units | Description |
| --- | --- | --- | --- |
| wepp_id | int32 |  |  |
| ofe_id | int16 |  |  |
| year | int16 |  |  |
| sim_day_index | int32 |  | 1-indexed simulation day |
| julian | int16 |  |  |
| month | int8 |  |  |
| day_of_month | int8 |  |  |
| water_year | int16 |  |  |
| OFE | int16 |  |  |
| Poros | double | % | Soil porosity |
| Keff | double | mm/hr | Effective hydraulic conductivity |
| Suct | double | mm | Suction across wetting front |
| FC | double | mm/mm | Field capacity |
| WP | double | mm/mm | Wilting point |
| Rough | double | mm | Surface roughness |
| Ki | double | adjsmt | Interrill erodibility adjustment factor |
| Kr | double | adjsmt | Rill erodibility adjustment factor |
| Tauc | double | adjsmt | Critical shear stress adjustment factor |
| Saturation | double | frac | Saturation as fraction (10mm profile) |
| TSW | double | mm | Total soil water |
| TSMF | double | frac | True soil moisture fraction (full profile) |

Preview:

wepp_id | ofe_id | year | sim_day_index | julian | month | day_of_month | water_year | OFE | Poros | Keff | Suct | FC | WP | Rough | Ki | Kr | Tauc | Saturation | TSW | TSMF
--- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | ---
 |  |  |  |  |  |  |  |  | % | mm/hr | mm | mm/mm | mm/mm | mm | adjsmt | adjsmt | adjsmt | frac | mm | frac
1 | 1 | 1980 | 1 | 1 | 1 | 1 | 1980 | 1 | 64.02 | 60 | 4.75 | 0.14 | 0.1 | 100 | 0.03 | 0.13 | 2 | 0.28 | 17.61 |
1 | 1 | 1980 | 2 | 2 | 1 | 2 | 1980 | 1 | 64.02 | 60 | 7.66 | 0.14 | 0.1 | 100 | 0.03 | 0.13 | 2 | 0.24 | 15.09 |
1 | 1 | 1980 | 3 | 3 | 1 | 3 | 1980 | 1 | 64.02 | 60 | 9.55 | 0.14 | 0.1 | 100 | 0.03 | 0.13 | 2 | 0.22 | 14.22 |

## Watershed Products

### `totalwatsed3.parquet`

Derived daily watershed summary from hillslope PASS/WAT; in MOFE runs latqcc uses only the outlet-facing (last) OFE per hillslope-day to avoid internal routing double-counting.

| Column | Type | Units | Description |
| --- | --- | --- | --- |
| year | int16 |  |  |
| sim_day_index | int32 |  |  |
| julian | int16 |  |  |
| month | int8 |  |  |
| day_of_month | int8 |  |  |
| water_year | int16 |  |  |
| runvol | double | m^3 | Runoff volume |
| sbrunv | double | m^3 | Subsurface runoff volume |
| tdet | double | kg | Total detachment |
| tdep | double | kg | Total deposition |
| seddep_1 | double | kg | Sediment Class 1 deposition |
| seddep_2 | double | kg | Sediment Class 2 deposition |
| seddep_3 | double | kg | Sediment Class 3 deposition |
| seddep_4 | double | kg | Sediment Class 4 deposition |
| seddep_5 | double | kg | Sediment Class 5 deposition |
| sed_del | double | kg | Total sediment delivery (sum of class masses) |
| sed_vol_conc | double | m^3/m^3 | Total volumetric sediment concentration (solids volume divided by runoff volume) |
| Area | double | m^2 | Area that depths apply over |
| P | double | m^3 | Precipitation volume |
| RM | double | m^3 | Rainfall+Irrigation+Snowmelt volume |
| Q | double | m^3 | Daily runoff over effective length volume |
| Dp | double | m^3 | Deep percolation volume |
| latqcc | double | m^3 | Lateral subsurface flow volume |
| QOFE | double | m^3 | Daily runoff scaled to single OFE volume |
| Ep | double | m^3 | Plant transpiration volume |
| Es | double | m^3 | Soil evaporation volume |
| Er | double | m^3 | Residue evaporation volume |
| UpStrmQ | double | mm | Runon added to OFE depth |
| SubRIn | double | mm | Subsurface runon added to OFE depth |
| Total-Soil Water | double | mm | Unfrozen water in soil profile depth |
| SoilWaterTotal | double | mm | Area-weighted full-profile soil water depth (watcon + frozwt) |
| ProfileDepth | double | mm | Area-weighted full soil profile depth (solthk(nsl)) |
| ProfilePorosityCap | double | mm | Area-weighted full-profile porosity storage capacity (sum(por * dg)) |
| ProfileFCStore | double | mm | Area-weighted full-profile field-capacity storage (sum(thetfc * dg)) |
| ProfileWPStore | double | mm | Area-weighted full-profile wilting-point storage (sum(thetdr * dg)) |
| InterceptionStorage | double | mm | Area-weighted plant/residue interception carryover storage depth (pintlv + resint) |
| TSMF | double | frac | Area-weighted true soil moisture fraction (full profile) |
| frozwt | double | mm | Frozen water in soil profile depth |
| Snow-Water | double | mm | Water in surface snow depth |
| QRain | double | mm | Area-weighted rain-generated runoff depth from element partitioning |
| QSnow | double | mm | Area-weighted snow-generated runoff depth from element partitioning |
| Tile | double | mm | Tile drainage depth |
| Irr | double | mm | Irrigation depth |
| Precipitation | double | mm | Precipitation depth |
| Rain+Melt | double | mm | Rainfall+Irrigation+Snowmelt depth |
| Percolation | double | mm | Deep percolation depth |
| Lateral Flow | double | mm | Lateral subsurface flow depth |
| Runoff | double | mm | Daily runoff depth from PASS runoff volume |
| Transpiration | double | mm | Plant transpiration depth |
| Evaporation | double | mm | Soil + residue evaporation depth |
| ET | double | mm | Total evapotranspiration depth |
| Interception | double | mm | Daily canopy/residue interception flux depth (optional producer-authoritative outflow) |
| Baseflow | double | mm | Baseflow depth |
| Aquifer losses | double | mm | Aquifer losses depth |
| Reservoir Volume | double | mm | Groundwater storage depth |
| Streamflow | double | mm | Streamflow depth |
| wind_transport | double | tonne | Ash transported by wind (total mass) |
| wind_transport_per_ha | double | tonne/ha | Ash transported by wind per unit area |
| wind_transport_black | double | tonne | Black ash transported by wind (total mass) |
| wind_transport_black_per_ha | double | tonne/ha | Black ash transported by wind per unit area over black ash hillslopes |
| wind_transport_white | double | tonne | White ash transported by wind (total mass) |
| wind_transport_white_per_ha | double | tonne/ha | White ash transported by wind per unit area over white ash hillslopes |
| water_transport | double | tonne | Ash transported by water (total mass) |
| water_transport_per_ha | double | tonne/ha | Ash transported by water per unit area |
| water_transport_black | double | tonne | Black ash transported by water (total mass) |
| water_transport_black_per_ha | double | tonne/ha | Black ash transported by water per unit area over black ash hillslopes |
| water_transport_white | double | tonne | White ash transported by water (total mass) |
| water_transport_white_per_ha | double | tonne/ha | White ash transported by water per unit area over white ash hillslopes |
| ash_transport | double | tonne | Total ash transported (wind + water) |
| ash_transport_per_ha | double | tonne/ha | Total ash transported per unit area |
| ash_transport_black | double | tonne | Black ash transported by wind + water (total mass) |
| ash_transport_black_per_ha | double | tonne/ha | Black ash transported per unit area over black ash hillslopes |
| ash_transport_white | double | tonne | White ash transported by wind + water (total mass) |
| ash_transport_white_per_ha | double | tonne/ha | White ash transported per unit area over white ash hillslopes |
| transportable_ash | double | tonne | Ash mass still available for transport |
| transportable_ash_per_ha | double | tonne/ha | Ash mass still available for transport per unit area |
| ash_vol_conc | double | m^3/m^3 | Ash volumetric concentration (solids volume divided by runoff volume) |
| sed+ash_vol_conc | double | m^3/m^3 | Sediment + ash volumetric concentration (total solids volume divided by runoff volume) |
| ash_black_pct_by_vol | double | percent | Fraction of ash solids volume that is black ash (percent of total ash volume) |

Preview:

year | sim_day_index | julian | month | day_of_month | water_year | runvol | sbrunv | tdet | tdep | seddep_1 | seddep_2 | seddep_3 | seddep_4 | seddep_5 | sed_del | sed_vol_conc | Area | P | RM | Q | Dp | latqcc | QOFE | Ep | Es | Er | UpStrmQ | SubRIn | Total-Soil Water | SoilWaterTotal | ProfileDepth | ProfilePorosityCap | ProfileFCStore | ProfileWPStore | InterceptionStorage | TSMF | frozwt | Snow-Water | QRain | QSnow | Tile | Irr | Precipitation | Rain+Melt | Percolation | Lateral Flow | Runoff | Transpiration | Evaporation | ET | Interception | Baseflow | Aquifer losses | Reservoir Volume | Streamflow | wind_transport | wind_transport_per_ha | wind_transport_black | wind_transport_black_per_ha | wind_transport_white | wind_transport_white_per_ha | water_transport | water_transport_per_ha | water_transport_black | water_transport_black_per_ha | water_transport_white | water_transport_white_per_ha | ash_transport | ash_transport_per_ha | ash_transport_black | ash_transport_black_per_ha | ash_transport_white | ash_transport_white_per_ha | transportable_ash | transportable_ash_per_ha | ash_vol_conc | sed+ash_vol_conc | ash_black_pct_by_vol
--- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | ---
 |  |  |  |  |  | m^3 | m^3 | kg | kg | kg | kg | kg | kg | kg | kg | m^3/m^3 | m^2 | m^3 | m^3 | m^3 | m^3 | m^3 | m^3 | m^3 | m^3 | m^3 | mm | mm | mm | mm | mm | mm | mm | mm | mm | frac | mm | mm | mm | mm | mm | mm | mm | mm | mm | mm | mm | mm | mm | mm | mm | mm | mm | mm | mm | tonne | tonne/ha | tonne | tonne/ha | tonne | tonne/ha | tonne | tonne/ha | tonne | tonne/ha | tonne | tonne/ha | tonne | tonne/ha | tonne | tonne/ha | tonne | tonne/ha | tonne | tonne/ha | m^3/m^3 | m^3/m^3 | percent
1980 | 1 | 1 | 1 | 1 | 1980 | 0 | 563917 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 2.66598e+08 | 0 | 0 | 0 | 11634.4 | 564044 | 0 | 204676 | 9882.92 | 0 | 0 | 0 | 233.709 |  |  |  |  |  |  |  | 0 | 0 |  |  | 0 | 0 | 0 | 0 | 0.0436402 | 2.11571 | 0 | 0.767733 | 0.0370706 | 0.804803 | 0 | 0 | 0 | 0 | 2.11571 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0
1980 | 2 | 2 | 1 | 2 | 1980 | 0 | 643992 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 2.66598e+08 | 0 | 0 | 0 | 11634.4 | 644059 | 0 | 294604 | 18116.9 | 0 | 0 | 0 | 230.079 |  |  |  |  |  |  |  | 0 | 0 |  |  | 0 | 0 | 0 | 0 | 0.0436402 | 2.41585 | 0 | 1.10505 | 0.0679559 | 1.17301 | 0 | 0.00174561 | 0 | 0.0436402 | 2.41759 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0
1980 | 3 | 3 | 1 | 3 | 1980 | 0 | 569827 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 2.66598e+08 | 0 | 0 | 0 | 11634.4 | 569893 | 0 | 314011 | 19360.9 | 0 | 0 | 0 | 226.649 |  |  |  |  |  |  |  | 0 | 0 |  |  | 0 | 0 | 0 | 0 | 0.0436402 | 2.13765 | 0 | 1.17785 | 0.0726223 | 1.25047 | 0 | 0.00342139 | 0 | 0.0855348 | 2.14107 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0
