//! Source-qualified modeled-area Roads reduction. Raw profiles remain immutable.
use super::*;

pub const POLICY: &str = "roads_modeled_area_v1";
// WAT path, native ID, canonical target ID, represented area, optional SOIL/ELEMENT.
pub type WaterSource = (String, i64, i64, f64, Option<String>, Option<String>);

struct Geometry {
    ofes: BTreeMap<i64, f64>,
    outlets: BTreeMap<Key, f64>,
}
impl Geometry {
    fn area(&self) -> f64 {
        self.ofes.values().sum()
    }
}

fn geometry(source: &Source, hill: i64) -> Result<Geometry, Error> {
    let groups = source
        .hills
        .get(&hill)
        .ok_or_else(|| err(format!("missing Roads WAT source {hill}")))?;
    let mut names = source.dates()?;
    names.extend(
        [
            source
                .ofe()
                .ok_or_else(|| err("Roads WAT requires OFE identity"))?,
            "Area",
            "Q",
            "latqcc",
        ]
        .map(str::to_string),
    );
    let validation_start = names.len();
    names.extend(WAT.iter().filter(|n| source.has(n)).map(|n| n.to_string()));
    let mut ofes = BTreeMap::new();
    let mut rows: BTreeMap<Key, BTreeMap<i64, (Option<f64>, Option<f64>)>> = BTreeMap::new();
    source.scan(groups, &names, |b| {
        for r in 0..b.rows {
            if b.int(0, r)? != hill {
                continue;
            }
            for col in validation_start..names.len() {
                if b.val(col, r).is_some_and(|v| v.is_infinite()) {
                    return Err(err("infinite Roads WAT value"));
                }
            }
            validate_date(b.key(r)?)?;
            let ofe = b.int(7, r)?;
            if ofe <= 0 {
                return Err(err("Roads WAT OFE must be positive"));
            }
            let area = b
                .val(8, r)
                .filter(|v| v.is_finite() && *v > 0.0)
                .ok_or_else(|| err("Roads WAT area must be finite and positive"))?;
            if ofes.insert(ofe, area).is_some_and(|old| old != area) {
                return Err(err("Roads WAT OFE area is not stationary"));
            }
            if rows
                .entry(b.key(r)?)
                .or_default()
                .insert(ofe, (b.val(9, r), b.val(10, r)))
                .is_some()
            {
                return Err(err("duplicate Roads WAT native row"));
            }
        }
        Ok(())
    })?;
    let terminal = *ofes
        .keys()
        .next_back()
        .ok_or_else(|| err("empty Roads WAT profile"))?;
    if ofes.keys().copied().ne(1..=terminal) {
        return Err(err("incomplete Roads WAT OFE topology"));
    }
    let mut outlets = BTreeMap::new();
    let area: f64 = ofes.values().sum();
    if !area.is_finite() {
        return Err(err("Roads modeled area overflow"));
    }
    let mut previous_sim_day = None;
    for (key, daily) in rows {
        if previous_sim_day.is_some_and(|day| key[1] != day + 1) {
            return Err(err("missing or duplicate Roads simulation day"));
        }
        previous_sim_day = Some(key[1]);

        if daily.keys().ne(ofes.keys()) {
            return Err(err("incomplete Roads WAT daily OFE topology"));
        }
        let (q, lat) = daily[&terminal];
        let q = q
            .filter(|v| v.is_finite())
            .ok_or_else(|| err("invalid Roads terminal Q"))?;
        lat.filter(|v| v.is_finite())
            .ok_or_else(|| err("invalid Roads terminal lateral flow"))?;
        outlets.insert(key, q * area * 0.001);
    }
    Ok(Geometry { ofes, outlets })
}

pub fn areas(path: &Path, ids: Vec<i64>) -> Result<Vec<(i64, Vec<(i64, f64)>)>, Error> {
    let mut source = Source::open(path, Some(&WAT))?;
    let selected = Some(ids.iter().copied().collect());
    source.index(&selected)?;
    ids.into_iter()
        .map(|id| Ok((id, geometry(&source, id)?.ofes.into_iter().collect())))
        .collect()
}

fn scaled<const N: usize>(source: Sums<N>, scale: f64) -> Sums<N> {
    Sums {
        values: source.values.map(|v| v * scale),
        seen: source.seen,
    }
}

fn destination_identity(path: &Path) -> Result<PathBuf, Error> {
    if path.exists() {
        std::fs::canonicalize(path).map_err(|e| Error::io(path, e))
    } else {
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let parent = std::fs::canonicalize(parent).map_err(|e| Error::io(parent, e))?;
        Ok(parent.join(
            path.file_name()
                .ok_or_else(|| err("missing output filename"))?,
        ))
    }
}

fn validate_alias(output: &Path, inputs: &[&Path]) -> Result<(), Error> {
    let identity = destination_identity(output)?;
    for input in inputs {
        if identity == destination_identity(input)? {
            return Err(err(
                "Roads output must not alias an input or another output",
            ));
        }
    }
    Ok(())
}

fn validate_date(key: Key) -> Result<(), Error> {
    if key[0] < 1
        || key[0] > i16::MAX as i64
        || key[1] < 1
        || key[1] > i32::MAX as i64
        || !(1..=366).contains(&key[2])
        || !(1..=12).contains(&key[3])
        || !(1..=31).contains(&key[4])
        || key[5] < key[0]
        || key[5] > key[0] + 1
        || key[5] > i16::MAX as i64
    {
        return Err(err("invalid Roads calendar key"));
    }
    let month_days = [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    if key[4] > month_days[(key[3] - 1) as usize] {
        return Err(err("invalid Roads day of month"));
    }
    Ok(())
}

pub fn produce(
    pass_path: &Path,
    sources: &[WaterSource],
    output: &Path,
    accounting: &Path,
    gwstorage: f64,
    bfcoeff: f64,
    dscoeff: f64,
    version: &VersionInfo,
    pandas_metadata: Option<&str>,
) -> Result<WriteSummary, Error> {
    if sources.is_empty() {
        return Err(err("Roads accounting requires explicit source profiles"));
    }
    if sources.iter().any(|s| s.2 <= 0 || s.2 > i32::MAX as i64) {
        return Err(err("invalid Roads canonical target ID"));
    }
    if ![gwstorage, bfcoeff, dscoeff].iter().all(|v| v.is_finite()) {
        return Err(err("nonfinite Roads groundwater parameters"));
    }
    let mut inputs = vec![pass_path];
    for (wat, _, _, _, soil, element) in sources {
        inputs.push(Path::new(wat));
        inputs.extend(soil.iter().chain(element).map(Path::new));
    }
    validate_alias(output, &inputs)?;
    validate_alias(accounting, &inputs)?;
    validate_alias(output, &[accounting])?;
    let mut opened = HashMap::new();
    for (wat, _, _, _, soil, element) in sources {
        for path in std::iter::once(wat).chain(soil).chain(element) {
            if !opened.contains_key(path) {
                let mut src = Source::open(Path::new(path), None)?;
                src.index(&None)?;
                opened.insert(path.clone(), src);
            }
        }
    }
    for (_, _, _, _, soil, element) in sources {
        for (path, metrics) in [(soil, &["TSMF"][..]), (element, &["QRain", "QSnow"][..])] {
            if let Some(path) = path {
                let src = &opened[path];
                if src.ofe().is_none()
                    || !metrics.iter().any(|m| src.has(m))
                    || !["year", "julian", "month", "day_of_month", "water_year"]
                        .iter()
                        .all(|n| src.has(n))
                    || (metrics == &["TSMF"] && src.day().is_err())
                {
                    return Err(err("malformed optional Roads source schema"));
                }
            }
        }
    }
    let mut seen = HashSet::new();
    let mut expected: Option<BTreeSet<Key>> = None;
    let mut days: HashMap<Key, Box<Daily>> = HashMap::new();
    let mut schema = crate::schema::hill_wat_schema(version);
    let mut metadata = schema.metadata().clone();
    metadata.insert("roads_accounting_policy".into(), POLICY.into());
    metadata.insert("derived_accounting".into(), "true".into());
    schema = schema.with_metadata(metadata);
    let mut accounting_sink = ParquetSink::try_new(accounting, schema.clone())?;
    let mut ordered: Vec<_> = sources.iter().collect();
    ordered.sort_by_key(|s| s.2);
    let targets: BTreeSet<_> = sources.iter().map(|s| s.2).collect();
    let mut current_target = ordered[0].2;
    let mut target_days: HashMap<Key, Box<Daily>> = HashMap::new();
    for (wat, id, target, represented, soil, element) in ordered {
        if *target != current_target {
            append_accounting(
                &mut accounting_sink,
                &schema,
                current_target,
                std::mem::take(&mut target_days),
            )?;
            current_target = *target;
        }
        let identity = (
            std::fs::canonicalize(wat).map_err(|e| Error::io(Path::new(wat), e))?,
            *id,
        );
        if !seen.insert(identity) {
            return Err(err("duplicate Roads source identity"));
        }
        if !represented.is_finite() || *represented < 0.0 {
            return Err(err("invalid Roads represented area"));
        }
        let src = &opened[wat];
        let geom = geometry(src, *id)?;
        let dates: BTreeSet<_> = geom.outlets.keys().copied().collect();
        if expected.as_ref().is_some_and(|e| e != &dates) {
            return Err(err("Roads source calendars differ"));
        }
        expected = Some(dates);
        let weight = represented / geom.area();
        // Zero baseline remainder is validated but contributes no response/support.
        if weight == 0.0 {
            continue;
        }
        let mut local = HashMap::new();
        aggregate_hill(
            src,
            soil.as_ref().map(|p| &opened[p]),
            element.as_ref().map(|p| &opened[p]),
            *id,
            &mut local,
        )?;
        for (key, d) in local {
            let dest = days.entry(key).or_default();
            dest.has_wat = true;
            dest.wat.merge(scaled(d.wat, weight));
            dest.weights.merge(scaled(d.weights, weight));
            let report = target_days.entry(key).or_default();
            let mut physical = d.wat;
            physical.values[3] = *geom
                .outlets
                .get(&key)
                .ok_or_else(|| err("optional Roads source date does not match WAT"))?;
            physical.seen |= 1 << 3;
            report.wat.merge(scaled(physical, weight));
        }
    }
    append_accounting(&mut accounting_sink, &schema, current_target, target_days)?;
    let selected = Some(targets.iter().copied().collect::<HashSet<_>>());
    let mut pass = Source::open(pass_path, Some(&PASS))?;
    pass.index(&selected)?;
    if pass.hills.keys().copied().collect::<BTreeSet<_>>() != targets {
        return Err(err("missing canonical Roads PASS targets"));
    }
    // Validate each canonical PASS once before aggregation; standalone contributors are excluded.
    for id in &targets {
        let mut names = pass.dates()?;
        names.extend(PASS.map(str::to_string));
        let mut dates = BTreeSet::new();
        pass.scan(&pass.hills[id], &names, |b| {
            for r in 0..b.rows {
                if b.int(0, r)? != *id {
                    continue;
                }
                validate_date(b.key(r)?)?;
                if !dates.insert(b.key(r)?) {
                    return Err(err("duplicate canonical PASS day"));
                }
                if (7..names.len()).any(|c| !b.val(c, r).is_some_and(|v| v.is_finite())) {
                    return Err(err("invalid required canonical PASS value"));
                }
            }
            Ok(())
        })?;
        if expected.as_ref() != Some(&dates) {
            return Err(err("canonical PASS and WAT calendars differ"));
        }
    }
    aggregate_pass(&pass, &pass.all(), &selected, &mut days)?;
    for d in days.values() {
        if d.wat
            .values
            .iter()
            .chain(&d.pass.values)
            .chain(&d.weights.values)
            .any(|v| v.is_infinite())
        {
            return Err(err(
                "Roads accounting arithmetic overflow or infinite input",
            ));
        }
    }

    let accounting = accounting_sink.finish_staged()?;
    let total = stage_days(
        days,
        output,
        gwstorage,
        bfcoeff,
        dscoeff,
        version,
        &[],
        pandas_metadata,
        Some(POLICY),
    )?;
    Ok(commit_staged(vec![total, accounting])?.remove(0))
}

fn append_accounting(
    sink: &mut ParquetSink,
    schema: &arrow_schema::Schema,
    id: i64,
    days: HashMap<Key, Box<Daily>>,
) -> Result<(), Error> {
    if days.is_empty() {
        return Err(err("Roads target has no positive represented area"));
    }
    for d in days.values() {
        if d.wat.values.iter().any(|v| v.is_infinite()) {
            return Err(err("Roads physical accounting overflow"));
        }
    }
    let mut days: Vec<_> = days.into_iter().collect();
    days.sort_by_key(|(k, _)| *k);
    for batch in days.chunks(BATCH_ROWS) {
        let mut arrays: Vec<Box<dyn Array>> = Vec::new();
        for field in schema.fields() {
            let name = field.name().as_str();
            let values: Vec<Option<f64>> = batch
                .iter()
                .map(|(key, d)| {
                    let value = match name {
                        "wepp_id" => id as f64,
                        "ofe_id" | "OFE" => 1.0,
                        "Area" => d.wat.get(0),
                        _ => {
                            if let Some(i) = DATE.iter().position(|n| *n == name) {
                                key[i] as f64
                            } else if let Some(i) = WAT.iter().position(|n| *n == name) {
                                depth(d.wat.get(i), d.wat.get(0))
                            } else {
                                f64::NAN
                            }
                        }
                    };
                    if value.is_nan() {
                        None
                    } else {
                        Some(value)
                    }
                })
                .collect();
            macro_rules! ints {
                ($ty:ty, $native:ty) => {
                    Box::new(<$ty>::from(
                        values
                            .into_iter()
                            .map(|v| v.map(|v| v as $native))
                            .collect::<Vec<_>>(),
                    )) as Box<dyn Array>
                };
            }
            arrays.push(match field.data_type() {
                DataType::Int8 => ints!(Int8Array, i8),
                DataType::Int16 => ints!(Int16Array, i16),
                DataType::Int32 => ints!(Int32Array, i32),
                _ => Box::new(Float64Array::from(values)),
            });
        }
        sink.write_chunk(Chunk::new(arrays))?;
    }
    Ok(())
}
