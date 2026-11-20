# Vilnius evacuation simulator

A desktop Rust port of the Vilnius evacuation hackathon prototype. It runs a fixed scenario with 500 agents, five example destinations, and the bundled Vilnius street network.

## Run on Windows

Download the x64 ZIP from [Releases](https://github.com/AdrianMoroz/vilnius-evacuation-rust/releases), extract it, and open VilniusEvacuation.exe. The program loads its map and calculates routes locally.

| Action | Control |
| --- | --- |
| Play or pause | Paleisti / Pristabdyti, or Space |
| Restart with the same agent positions | Atkurti, or R |
| Pan and zoom | Drag the map and scroll |
| Inspect a route | Click an agent, or enter its number and click Rodyti |
| Close or reopen a segment | Right-click the segment, or use Pasirinkti kelio atkarpą |
| Fit the city to the window | Visas miestas |

The model uses fictional destinations and an illustrative movement speed. Vilnius publishes its real-world civil protection information separately. [Vilnius civil protection](https://vilnius.lt/savivaldybe/saugus-miestas/civiline-sauga)

## Build from source

Install stable Rust 1.88 or newer. On Windows, use the MSVC Rust toolchain and matching Visual C++ build tools.

```sh
cargo run --release --locked
cargo test --locked
```

The Windows workflow tests the scenario and packages the executable. The parity tests cover starting positions, arrival counts, reopening, and the route change for a sample closure.

## Map data

The bundled road graph is the same OpenStreetMap snapshot used by the browser prototype. [Data source and license](assets/README.md)
