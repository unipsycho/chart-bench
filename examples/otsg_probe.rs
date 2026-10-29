use csv_bench::ingest::{detect_parse_plan, load_csv_path, sniff_delimiter, RawTable};

fn main() {
    let path = std::path::Path::new(
        "/home/mike/Documents/AAI-Projects/csv-bench/samples/otsg2_header_sample.tsv",
    );
    let text = std::fs::read_to_string(path).unwrap();
    let d = sniff_delimiter(&text);
    println!("delim={d:?} {}", d as char);
    let mut rows = Vec::new();
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(false)
        .delimiter(d)
        .flexible(true)
        .from_reader(text.as_bytes());
    for rec in rdr.records() {
        let rec = rec.unwrap();
        rows.push(rec.iter().map(|s| s.to_string()).collect::<Vec<_>>());
    }
    for (i,r) in rows.iter().take(5).enumerate() {
        println!("row{i} len={} first4={:?}", r.len(), &r[..r.len().min(4)]);
    }
    let table = RawTable { rows };
    let plan = detect_parse_plan(&table);
    println!("plan headers={} data@{} width={} roles={:?}", plan.header_rows, plan.data_start_row, plan.width, plan.roles);
    println!("x={:?} {:?}", plan.x_axis.mode, plan.x_axis.column_index);
    let ds = load_csv_path(path).unwrap();
    for ch in ds.channels.iter().take(10) {
        println!("ch raw={} name={:?} units={:?} disp={:?}", ch.raw_column_index, ch.name, ch.units, ch.display_name);
    }
}
