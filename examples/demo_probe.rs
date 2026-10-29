use csv_bench::ingest::detect::{detect_parse_plan, RawTable};
use csv_bench::model::HeaderRole;
fn main() {
    let table = RawTable {
        rows: vec![
            vec!["Sensor".into(),"Temp".into(),"Temp".into(),"Pressure".into(),"Flow".into()],
            vec!["Node".into(),"TankA".into(),"TankB".into(),"TankA".into(),"Line1".into()],
            vec!["Units".into(),"°C".into(),"°C".into(),"kPa".into(),"L/min".into()],
            vec!["2024-10-01T08:00:00".into(),"21.1".into(),"22.0".into(),"101.3".into(),"3.20".into()],
        ],
    };
    let plan = detect_parse_plan(&table);
    println!("headers={} data@{} width={} roles={:?}", plan.header_rows, plan.data_start_row, plan.width, plan.roles);
}
