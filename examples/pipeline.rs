use dessert::{Engine, Event, State, Timestamp};
use std::{env, fs::File, io::Write, path::Path};

#[derive(Debug, Clone)]
struct SimState {
    wheat: u32,
    flour: u32,
    bread: u32,
    mills: usize,
    bakeries: usize,
    idle_mill_workers: u32,
    idle_bakery_workers: u32,
    deliveries_per_crop: u32,
    load_size_wheat: u32,
    crop_duration: f64,
    farm_distance_tiles: f64,
    farm_empty_speed_tiles_per_month: f64,
    farm_loaded_speed_tiles_per_month: f64,
    mill_distance_tiles: f64,
    mill_empty_speed_tiles_per_month: f64,
    mill_loaded_speed_tiles_per_month: f64,
    mill_job_time: f64,
    bakery_distance_tiles: f64,
    bakery_empty_speed_tiles_per_month: f64,
    bakery_loaded_speed_tiles_per_month: f64,
    bakery_job_time: f64,
    bakery_output_bread: u32,
}

#[derive(Debug, Clone, Copy)]
enum FarmEvent {
    // Empty walk to farm (from keep at start, or returning from stockpile):
    // if `remaining` is Some(n), then upon arrival we will dispatch another loaded walk with n-1 remaining.
    WalkEmptyToFarm { at: Timestamp, remaining: u32 },
    ArriveEmptyFarm { at: Timestamp, remaining: u32 },
    // Farming phase
    ProcessStart { at: Timestamp },
    ProcessEnd { at: Timestamp },
    // Loaded walk to stockpile and arrival (delivery)
    WalkLoadedToStockpile { at: Timestamp, remaining: u32 },
    ArriveLoadedToStockpile { at: Timestamp, remaining: u32 },
}

#[derive(Debug, Clone, Copy)]
enum MillEvent {
    WalkEmptyToStockpile { at: Timestamp },
    ArriveEmptyStockpile { at: Timestamp },
    WalkLoadedToMill { at: Timestamp },
    ArriveLoadedMill { at: Timestamp },
    ProcessStart { at: Timestamp },
    ProcessEnd { at: Timestamp },
    WalkLoadedToStockpile { at: Timestamp },
    ArriveLoadedStockpile { at: Timestamp },
    WalkEmptyToMill { at: Timestamp },
    ArriveEmptyMill { at: Timestamp },
}

#[derive(Debug, Clone, Copy)]
enum BakeryEvent {
    WalkEmptyToStockpile { at: Timestamp },
    ArriveEmptyStockpile { at: Timestamp },
    WalkLoadedToBakery { at: Timestamp },
    ArriveLoadedBakery { at: Timestamp },
    ProcessStart { at: Timestamp },
    ProcessEnd { at: Timestamp },
    WalkLoadedToGranary { at: Timestamp },
    ArriveLoadedGranary { at: Timestamp },
    WalkEmptyToBakery { at: Timestamp },
    ArriveEmptyBakery { at: Timestamp },
}

#[derive(Debug, Clone, Copy)]
enum PipelineEvent {
    Farm(FarmEvent),
    Mill(MillEvent),
    Bakery(BakeryEvent),
}

impl Event<SimState> for PipelineEvent {
    fn time(&self) -> Timestamp {
        match *self {
            PipelineEvent::Farm(ev) => match ev {
                FarmEvent::WalkEmptyToFarm { at, .. }
                | FarmEvent::ArriveEmptyFarm { at, .. }
                | FarmEvent::ProcessStart { at }
                | FarmEvent::ProcessEnd { at }
                | FarmEvent::WalkLoadedToStockpile { at, .. }
                | FarmEvent::ArriveLoadedToStockpile { at, .. } => at,
            },
            PipelineEvent::Mill(ev) => match ev {
                MillEvent::WalkEmptyToStockpile { at }
                | MillEvent::ArriveEmptyStockpile { at }
                | MillEvent::WalkLoadedToMill { at }
                | MillEvent::ArriveLoadedMill { at }
                | MillEvent::ProcessStart { at }
                | MillEvent::ProcessEnd { at }
                | MillEvent::WalkLoadedToStockpile { at }
                | MillEvent::ArriveLoadedStockpile { at }
                | MillEvent::WalkEmptyToMill { at }
                | MillEvent::ArriveEmptyMill { at } => at,
            },
            PipelineEvent::Bakery(ev) => match ev {
                BakeryEvent::WalkEmptyToStockpile { at }
                | BakeryEvent::ArriveEmptyStockpile { at }
                | BakeryEvent::WalkLoadedToBakery { at }
                | BakeryEvent::ArriveLoadedBakery { at }
                | BakeryEvent::ProcessStart { at }
                | BakeryEvent::ProcessEnd { at }
                | BakeryEvent::WalkLoadedToGranary { at }
                | BakeryEvent::ArriveLoadedGranary { at }
                | BakeryEvent::WalkEmptyToBakery { at }
                | BakeryEvent::ArriveEmptyBakery { at } => at,
            },
        }
    }

    fn execute(self, state: &mut State<SimState, PipelineEvent>) {
        match self {
            PipelineEvent::Farm(ev) => handle_farm_event(state, ev),
            PipelineEvent::Mill(ev) => handle_mill_event(state, ev),
            PipelineEvent::Bakery(ev) => handle_bakery_event(state, ev),
        }
    }
}

fn handle_farm_event(state: &mut State<SimState, PipelineEvent>, ev: FarmEvent) {
    match ev {
        FarmEvent::WalkEmptyToFarm { at, remaining } => {
            let t_walk = travel_time(
                state.state().farm_distance_tiles,
                state.state().farm_empty_speed_tiles_per_month,
            );
            let arrive_at = at + t_walk;
            state.schedule(PipelineEvent::Farm(FarmEvent::ArriveEmptyFarm {
                at: arrive_at,
                remaining,
            }));
        }
        FarmEvent::ArriveEmptyFarm { at, remaining } => {
            if remaining > 0 {
                state.schedule(PipelineEvent::Farm(FarmEvent::WalkLoadedToStockpile {
                    at,
                    remaining,
                }));
            } else {
                state.schedule(PipelineEvent::Farm(FarmEvent::ProcessStart { at }));
            }
        }
        FarmEvent::ProcessStart { at } => {
            let end_at = at + state.state().crop_duration;
            state.schedule(PipelineEvent::Farm(FarmEvent::ProcessEnd { at: end_at }));
        }
        FarmEvent::ProcessEnd { at } => {
            state.schedule(PipelineEvent::Farm(FarmEvent::WalkLoadedToStockpile {
                at,
                remaining: state.state().deliveries_per_crop,
            }));
        }
        FarmEvent::WalkLoadedToStockpile { at, remaining } => {
            let t = at
                + travel_time(
                    state.state().farm_distance_tiles,
                    state.state().farm_loaded_speed_tiles_per_month,
                );
            state.schedule(PipelineEvent::Farm(FarmEvent::ArriveLoadedToStockpile {
                at: t,
                remaining,
            }));
        }
        FarmEvent::ArriveLoadedToStockpile { at, remaining } => {
            let add = state.state().load_size_wheat;
            state.state_mut().wheat = state.state().wheat + add;
            try_start_mill_jobs(state);
            let next_remaining = if remaining > 1 { remaining - 1 } else { 0 };
            state.schedule(PipelineEvent::Farm(FarmEvent::WalkEmptyToFarm {
                at,
                remaining: next_remaining,
            }));
        }
    }
}

fn handle_mill_event(state: &mut State<SimState, PipelineEvent>, ev: MillEvent) {
    match ev {
        MillEvent::WalkEmptyToStockpile { at } => {
            let t = at
                + travel_time(
                    state.state().mill_distance_tiles,
                    state.state().mill_empty_speed_tiles_per_month,
                );
            state.schedule(PipelineEvent::Mill(MillEvent::ArriveEmptyStockpile {
                at: t,
            }));
        }
        MillEvent::ArriveEmptyStockpile { at } => {
            if state.state().wheat > 0 {
                // Consume wheat now and head to mill loaded
                state.state_mut().wheat -= 1;
                state.schedule(PipelineEvent::Mill(MillEvent::WalkLoadedToMill { at }));
            } else {
                // Nothing to pick up, return empty
                state.schedule(PipelineEvent::Mill(MillEvent::WalkEmptyToMill { at }));
            }
        }
        MillEvent::WalkLoadedToMill { at } => {
            let t = at
                + travel_time(
                    state.state().mill_distance_tiles,
                    state.state().mill_loaded_speed_tiles_per_month,
                );
            state.schedule(PipelineEvent::Mill(MillEvent::ArriveLoadedMill { at: t }));
        }
        MillEvent::ArriveLoadedMill { at } => {
            state.schedule(PipelineEvent::Mill(MillEvent::ProcessStart { at }));
        }
        MillEvent::ProcessStart { at } => {
            state.schedule(PipelineEvent::Mill(MillEvent::ProcessEnd {
                at: at + state.state().mill_job_time,
            }));
        }
        MillEvent::ProcessEnd { at } => {
            // Start loaded walk back to stockpile (travel handled in the Walk event)
            state.schedule(PipelineEvent::Mill(MillEvent::WalkLoadedToStockpile { at }));
        }
        MillEvent::WalkLoadedToStockpile { at } => {
            let t = at
                + travel_time(
                    state.state().mill_distance_tiles,
                    state.state().mill_loaded_speed_tiles_per_month,
                );
            state.schedule(PipelineEvent::Mill(MillEvent::ArriveLoadedStockpile {
                at: t,
            }));
        }
        MillEvent::ArriveLoadedStockpile { at: _ } => {
            state.state_mut().flour += 1;
            try_start_bakery_jobs(state);
            let t = state.now()
                + travel_time(
                    state.state().mill_distance_tiles,
                    state.state().mill_empty_speed_tiles_per_month,
                );
            state.schedule(PipelineEvent::Mill(MillEvent::WalkEmptyToMill { at: t }));
        }
        MillEvent::WalkEmptyToMill { at } => {
            let t = at
                + travel_time(
                    state.state().mill_distance_tiles,
                    state.state().mill_empty_speed_tiles_per_month,
                );
            state.schedule(PipelineEvent::Mill(MillEvent::ArriveEmptyMill { at: t }));
        }
        MillEvent::ArriveEmptyMill { at: _ } => {
            state.state_mut().idle_mill_workers += 1;
            try_start_mill_jobs(state);
        }
    }
}

fn handle_bakery_event(state: &mut State<SimState, PipelineEvent>, ev: BakeryEvent) {
    match ev {
        BakeryEvent::WalkEmptyToStockpile { at } => {
            let t = at
                + travel_time(
                    state.state().bakery_distance_tiles,
                    state.state().bakery_empty_speed_tiles_per_month,
                );
            state.schedule(PipelineEvent::Bakery(BakeryEvent::ArriveEmptyStockpile {
                at: t,
            }));
        }
        BakeryEvent::ArriveEmptyStockpile { at } => {
            if state.state().flour > 0 {
                // Consume flour now and carry to bakery
                state.state_mut().flour -= 1;
                state.schedule(PipelineEvent::Bakery(BakeryEvent::WalkLoadedToBakery {
                    at,
                }));
            } else {
                // Nothing to pick up; return empty to bakery
                state.schedule(PipelineEvent::Bakery(BakeryEvent::WalkEmptyToBakery { at }));
            }
        }
        BakeryEvent::WalkLoadedToBakery { at } => {
            let t = at
                + travel_time(
                    state.state().bakery_distance_tiles,
                    state.state().bakery_loaded_speed_tiles_per_month,
                );
            state.schedule(PipelineEvent::Bakery(BakeryEvent::ArriveLoadedBakery {
                at: t,
            }));
        }
        BakeryEvent::ArriveLoadedBakery { at } => {
            state.schedule(PipelineEvent::Bakery(BakeryEvent::ProcessStart { at }));
        }
        BakeryEvent::ProcessStart { at } => {
            state.schedule(PipelineEvent::Bakery(BakeryEvent::ProcessEnd {
                at: at + state.state().bakery_job_time,
            }));
        }
        BakeryEvent::ProcessEnd { at } => {
            // Start loaded walk to granary (travel handled in Walk event)
            state.schedule(PipelineEvent::Bakery(BakeryEvent::WalkLoadedToGranary {
                at,
            }));
        }
        BakeryEvent::WalkLoadedToGranary { at } => {
            let t = at
                + travel_time(
                    state.state().bakery_distance_tiles,
                    state.state().bakery_loaded_speed_tiles_per_month,
                );
            state.schedule(PipelineEvent::Bakery(BakeryEvent::ArriveLoadedGranary {
                at: t,
            }));
        }
        BakeryEvent::ArriveLoadedGranary { at: _ } => {
            state.state_mut().bread += state.state().bakery_output_bread;
            let t = state.now()
                + travel_time(
                    state.state().bakery_distance_tiles,
                    state.state().bakery_empty_speed_tiles_per_month,
                );
            state.schedule(PipelineEvent::Bakery(BakeryEvent::WalkEmptyToBakery {
                at: t,
            }));
        }
        BakeryEvent::WalkEmptyToBakery { at } => {
            let t = at
                + travel_time(
                    state.state().bakery_distance_tiles,
                    state.state().bakery_empty_speed_tiles_per_month,
                );
            state.schedule(PipelineEvent::Bakery(BakeryEvent::ArriveEmptyBakery {
                at: t,
            }));
        }
        BakeryEvent::ArriveEmptyBakery { at: _ } => {
            state.state_mut().idle_bakery_workers += 1;
            try_start_bakery_jobs(state);
        }
    }
}

fn travel_time(distance_tiles: f64, speed_tiles_per_month: f64) -> f64 {
    if speed_tiles_per_month <= 0.0 {
        0.0
    } else {
        distance_tiles / speed_tiles_per_month
    }
}

fn try_start_mill_jobs(state: &mut State<SimState, PipelineEvent>) {
    // Ensure idle workers reflect mill count
    let total_mill_workers = (state.state().mills as u32) * 3;
    if state.state().idle_mill_workers > total_mill_workers {
        state.state_mut().idle_mill_workers = total_mill_workers;
    }
    // Alert all idle workers if there is any wheat available.
    if state.state().wheat > 0 {
        while state.state().idle_mill_workers > 0 {
            state.state_mut().idle_mill_workers -= 1;
            state.schedule(PipelineEvent::Mill(MillEvent::WalkEmptyToStockpile {
                at: state.now(),
            }));
        }
    }
}

fn try_start_bakery_jobs(state: &mut State<SimState, PipelineEvent>) {
    // Ensure idle workers reflect bakery count
    let total_bakery_workers = state.state().bakeries as u32;
    if state.state().idle_bakery_workers > total_bakery_workers {
        state.state_mut().idle_bakery_workers = total_bakery_workers;
    }
    // Alert all idle bakery workers if any flour is available.
    if state.state().flour > 0 {
        while state.state().idle_bakery_workers > 0 {
            state.state_mut().idle_bakery_workers -= 1;
            state.schedule(PipelineEvent::Bakery(BakeryEvent::WalkEmptyToStockpile {
                at: state.now(),
            }));
        }
    }
}

fn parse_arg<T: std::str::FromStr>(name: &str, default: T) -> T {
    let mut args = env::args().skip(1);
    while let Some(k) = args.next() {
        if k == name {
            if let Some(v) = args.next() {
                if let Ok(parsed) = v.parse::<T>() {
                    return parsed;
                }
            }
        }
    }
    default
}

fn parse_arg_str(name: &str) -> Option<String> {
    let mut args = env::args().skip(1);
    while let Some(k) = args.next() {
        if k == name {
            return args.next();
        }
    }
    None
}

fn write_history_csv<P: AsRef<Path>>(
    path: P,
    history: &[State<SimState, PipelineEvent>],
) -> std::io::Result<()> {
    let mut f = File::create(path)?;
    writeln!(f, "months,wheat,flour,bread")?;
    for st in history {
        writeln!(
            f,
            "{:.6},{},{},{}",
            st.now(),
            st.state().wheat,
            st.state().flour,
            st.state().bread
        )?;
    }
    Ok(())
}

fn main() {
    let farms: usize = parse_arg("--farms", 2usize);
    let mills: usize = parse_arg("--mills", 1usize);
    let bakeries: usize = parse_arg("--bakeries", 4usize);
    let months: f64 = parse_arg("--months", 60.0f64);

    // Parameters with defaults matching our earlier discussion
    let deliveries_per_crop: u32 = parse_arg("--deliveries-per-crop", 12u32);
    let load_size_wheat: u32 = parse_arg("--load-size", 2u32);
    let crop_duration: f64 = parse_arg("--crop-duration", 18.0f64);
    let mill_distance_tiles: f64 = parse_arg("--mill-distance", 10.0f64);
    let mill_empty_speed_tiles_per_month: f64 = parse_arg("--mill-empty-speed", 66.6f64);
    let mill_loaded_speed_tiles_per_month: f64 = parse_arg("--mill-loaded-speed", 50.0f64);
    let mill_job_time: f64 = parse_arg("--mill-job-time", 1.125f64);
    let bakery_distance_tiles: f64 = parse_arg("--bakery-distance", 10.0f64);
    // Speeds: parse legacy single-speed flags first for compatibility, then allow direction-specific overrides
    let bakery_default_speed: f64 = parse_arg("--bakery-walk-speed", 33.3f64);
    let bakery_empty_speed_tiles_per_month: f64 =
        parse_arg("--bakery-empty-speed", bakery_default_speed);
    let bakery_loaded_speed_tiles_per_month: f64 =
        parse_arg("--bakery-loaded-speed", bakery_default_speed);
    let bakery_job_time: f64 = parse_arg("--bakery-job-time", 3.0f64);
    let bakery_output_bread: u32 = parse_arg("--bakery-output", 8u32);
    // Default CSV outputs unless overridden via flags
    let csv_file: String =
        parse_arg_str("--csv-file").unwrap_or_else(|| "pipeline.csv".to_string());
    let events_file: String =
        parse_arg_str("--events-csv").unwrap_or_else(|| "pipeline_ev.csv".to_string());
    let farm_distance_tiles: f64 = parse_arg("--farm-distance", 100.0f64);
    let farm_default_speed: f64 = parse_arg("--farm-walk-speed", 33.3f64);
    let farm_empty_speed_tiles_per_month: f64 = parse_arg("--farm-empty-speed", farm_default_speed);
    let farm_loaded_speed_tiles_per_month: f64 =
        parse_arg("--farm-loaded-speed", farm_default_speed);

    let mut engine = Engine::<SimState, PipelineEvent>::new(SimState {
        wheat: 0,
        flour: 0,
        bread: 0,
        mills,
        bakeries,
        idle_mill_workers: (mills as u32) * 3,
        idle_bakery_workers: bakeries as u32,
        deliveries_per_crop,
        load_size_wheat,
        crop_duration,
        farm_distance_tiles,
        farm_empty_speed_tiles_per_month,
        farm_loaded_speed_tiles_per_month,
        mill_distance_tiles,
        mill_empty_speed_tiles_per_month,
        mill_loaded_speed_tiles_per_month,
        mill_job_time,
        bakery_distance_tiles,
        bakery_empty_speed_tiles_per_month,
        bakery_loaded_speed_tiles_per_month,
        bakery_job_time,
        bakery_output_bread,
    });

    for _ in 0..farms {
        engine.schedule(PipelineEvent::Farm(FarmEvent::WalkEmptyToFarm {
            at: 0.0,
            remaining: 0,
        }));
    }

    println!(
        "Pipeline simulation: farms={}, mills={}, bakeries={}, months={}",
        farms, mills, bakeries, months
    );
    engine.run_until(months);
    let s = engine.state();
    println!("End: wheat={} flour={} bread={}", s.wheat, s.flour, s.bread);

    if let Err(e) = write_history_csv(&csv_file, engine.history()) {
        eprintln!("Failed to write CSV '{}': {}", csv_file, e);
    } else {
        println!("Saved CSV to {}", csv_file);
    }
    if let Err(e) = write_events_csv(&events_file, engine.events()) {
        eprintln!("Failed to write events CSV '{}': {}", events_file, e);
    } else {
        println!("Saved events CSV to {}", events_file);
    }
}

fn write_events_csv<P: AsRef<Path>>(path: P, events: &[(f64, String)]) -> std::io::Result<()> {
    let mut f = File::create(path)?;
    writeln!(f, "months,event")?;
    for (t, name) in events {
        // Quote and escape event name to keep CSV well-formed (CSV escaping doubles quotes)
        let escaped = name.replace('"', "\"\"");
        writeln!(f, "{:.6},\"{}\"", t, escaped)?;
    }
    Ok(())
}
