use dessert::{Engine, Event, State, Timestamp};
use std::{env, fs::File, io::Write, path::Path};

#[derive(Debug, Default, Clone)]
struct FarmSimState {
    wheat: u32,
    deliveries_per_crop: u32,
    load_size: u32,
    crop_duration: f64,
    farm_distance_tiles: f64,
    worker_speed_tiles_per_month: f64, // 33.3 tiles/month
}

#[derive(Debug, Clone, Copy)]
enum FarmEvent {
    WalkToFarm { at: Timestamp },
    StartCrop { at: Timestamp },
    CropDone { at: Timestamp },
    WalkToStockpile { at: Timestamp, remaining: u32 },
    Deliver { at: Timestamp, remaining: u32 },
    WalkBackToFarm { at: Timestamp, remaining: u32 },
}

impl Event<FarmSimState> for FarmEvent {
    fn time(&self) -> Timestamp {
        match *self {
            FarmEvent::WalkToFarm { at } => at,
            FarmEvent::StartCrop { at } => at,
            FarmEvent::CropDone { at } => at,
            FarmEvent::WalkToStockpile { at, .. } => at,
            FarmEvent::Deliver { at, .. } => at,
            FarmEvent::WalkBackToFarm { at, .. } => at,
        }
    }

    fn execute(self, state: &mut State<FarmSimState, FarmEvent>) {
        match self {
            FarmEvent::WalkToFarm { at } => {
                let t_walk = if state.state().worker_speed_tiles_per_month > 0.0 {
                    state.state().farm_distance_tiles / state.state().worker_speed_tiles_per_month
                } else {
                    0.0
                };
                state.schedule(FarmEvent::StartCrop { at: at + t_walk });
            }
            FarmEvent::StartCrop { at } => {
                let done_at = at + state.state().crop_duration;
                state.schedule(FarmEvent::CropDone { at: done_at });
            }
            FarmEvent::CropDone { at } => {
                let remaining = state.state().deliveries_per_crop;
                state.schedule(FarmEvent::WalkToStockpile { at, remaining });
            }
            FarmEvent::WalkToStockpile { at, remaining } => {
                let t_walk = if state.state().worker_speed_tiles_per_month > 0.0 {
                    state.state().farm_distance_tiles / state.state().worker_speed_tiles_per_month
                } else {
                    0.0
                };
                state.schedule(FarmEvent::Deliver {
                    at: at + t_walk,
                    remaining,
                });
            }
            FarmEvent::Deliver { at, remaining } => {
                let load = state.state().load_size;
                state.state_mut().wheat += load;
                println!(
                    "t={:.2} mo | delivery: +{} wheat (total {})",
                    at,
                    load,
                    state.state().wheat
                );
                if remaining > 1 {
                    state.schedule(FarmEvent::WalkBackToFarm {
                        at,
                        remaining: remaining - 1,
                    });
                } else {
                    // After final delivery, walk back to farm and start next crop
                    state.schedule(FarmEvent::WalkToFarm { at });
                }
            }
            FarmEvent::WalkBackToFarm { at, remaining } => {
                let t_walk = if state.state().worker_speed_tiles_per_month > 0.0 {
                    state.state().farm_distance_tiles / state.state().worker_speed_tiles_per_month
                } else {
                    0.0
                };
                state.schedule(FarmEvent::WalkToStockpile {
                    at: at + t_walk,
                    remaining,
                });
            }
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
            if let Some(v) = args.next() {
                return Some(v);
            }
        }
    }
    None
}

fn main() {
    // Simple args: --farms N --months M [--deliveries-per-crop 12 --load-size 2 --crop-duration 18]
    let farms: usize = parse_arg("--farms", 2usize);
    let months: f64 = parse_arg("--months", 36.0f64);
    let ascii_plot: bool = parse_arg("--ascii-plot", 0u32) != 0; // pass 1 to enable
    let width: usize = parse_arg("--width", 80usize);
    let height: usize = parse_arg("--height", 20usize);
    let csv_file: Option<String> = parse_arg_str("--csv-file");
    let events_file: Option<String> = parse_arg_str("--events-csv");
    let farm_distance_tiles: f64 = parse_arg("--farm-distance", 0.0f64);
    let worker_speed_tiles_per_month: f64 = parse_arg("--farm-walk-speed", 33.3f64);
    let deliveries_per_crop: u32 = parse_arg("--deliveries-per-crop", 12u32);
    let load_size: u32 = parse_arg("--load-size", 2u32);
    let crop_duration: f64 = parse_arg("--crop-duration", 18.0f64);

    let mut engine = Engine::<FarmSimState, FarmEvent>::new(FarmSimState {
        wheat: 0,
        farms,
        deliveries_per_crop,
        load_size,
        crop_duration,
        farm_distance_tiles,
        worker_speed_tiles_per_month,
    });

    // Seed initial crops for each farm at t=0
    for _ in 0..farms {
        engine.schedule(FarmEvent::WalkToFarm { at: 0.0 });
    }

    println!(
        "Starting farm-only simulation: farms={}, horizon={} months (crop: {} mo, {} loads, +{} each, distance={} tiles, walk_speed={} tiles/mo)",
        farms, months, crop_duration, deliveries_per_crop, load_size, farm_distance_tiles, worker_speed_tiles_per_month
    );
    engine.run_until(months);
    println!(
        "\nEnd of simulation: wheat total = {}",
        engine.state().wheat
    );

    if let Some(path) = csv_file.as_deref() {
        if let Err(e) = write_history_csv(path, engine.history()) {
            eprintln!("Failed to write CSV '{}': {}", path, e);
        } else {
            println!("Saved CSV to {}", path);
        }
    }
    if let Some(path) = events_file.as_deref() {
        if let Err(e) = write_events_csv(path, engine.events()) {
            eprintln!("Failed to write events CSV '{}': {}", path, e);
        } else {
            println!("Saved events CSV to {}", path);
        }
    }

    if ascii_plot {
        println!("\nASCII plot of wheat over time ({}x{}):", width, height);
        ascii_plot_wheat(engine.history(), months, width, height);
    }
}

fn ascii_plot_wheat(
    history: &[State<FarmSimState, FarmEvent>],
    horizon: f64,
    width: usize,
    height: usize,
) {
    if history.is_empty() || width < 10 || height < 5 || horizon <= 0.0 {
        return;
    }
    let mut ymax = 0u32;
    for st in history {
        ymax = ymax.max(st.state().wheat);
    }
    let ymax = ymax.max(1);
    let mut grid = vec![vec![' '; width]; height];
    // Axes: x-axis bottom row
    for x in 0..width {
        grid[height - 1][x] = '-';
    }
    grid[height - 1][0] = '+';
    // Y-axis at left
    for y in 0..height {
        grid[y][0] = '|';
    }

    // Plot step-like series by sampling columns
    let mut idx = 0usize;
    for x in 0..width {
        let t = (x as f64) * horizon / (width.saturating_sub(1) as f64);
        while idx + 1 < history.len() && history[idx + 1].now() <= t {
            idx += 1;
        }
        let val = history[idx].state().wheat;
        let y = if ymax == 0 {
            0
        } else {
            (val as f64 * (height as f64 - 2.0) / ymax as f64).round() as usize
        };
        let y = (height - 2).saturating_sub(y);
        if x < width && y < height {
            grid[y][x] = '#';
        }
    }

    // Print with simple labels
    for y in 0..height {
        let row: String = grid[y].iter().collect();
        println!("{}", row);
    }
    println!(
        "0 mo{}{:>6.2} mo  ymax: {} wheat",
        " ".repeat(width.saturating_sub(19)),
        horizon,
        ymax
    );
}

fn write_history_csv<P: AsRef<Path>>(
    path: P,
    history: &[State<FarmSimState, FarmEvent>],
) -> std::io::Result<()> {
    let mut f = File::create(path)?;
    // Match Python plotter headers for compatibility
    writeln!(f, "months,wheat,flour,bread")?;
    for st in history {
        writeln!(f, "{:.6},{},0,0", st.now(), st.state().wheat)?;
    }
    Ok(())
}

fn write_events_csv<P: AsRef<Path>>(path: P, events: &[(f64, String)]) -> std::io::Result<()> {
    let mut f = File::create(path)?;
    writeln!(f, "months,event")?;
    for (t, name) in events {
        writeln!(f, "{:.6},{}", t, name)?;
    }
    Ok(())
}
