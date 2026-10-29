.PHONY: run release test inspect sync

run:
	cargo run

release:
	cargo run --release

test:
	cargo test

inspect:
	rustc tools/inspect_csv.rs -o /tmp/inspect_csv
	/tmp/inspect_csv samples/demo_multiline.csv

sync:
	bash scripts/sync-from-store.sh
