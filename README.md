# Air-Sim

Expansion chamber muffler simulator using the Transfer Matrix Method (TMM). Features a real-time GUI with geometry visualisation, transmission loss plotting, and audio playback of simulated exhaust sound.

## Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) (stable toolchain)

### Linux

Install system dependencies for the GUI (eframe/egui) and audio (cpal/ALSA).

**Debian / Ubuntu:**

```bash
sudo apt install libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev \
    libxkbcommon-dev libgtk-3-dev libasound2-dev pkg-config
```

**Arch / Manjaro:**

```bash
sudo pacman -S libxcb libxkbcommon gtk3 alsa-lib pkg-config
```

**Fedora:**

```bash
sudo dnf install libxcb-devel libxkbcommon-devel gtk3-devel alsa-lib-devel pkg-config
```

### Windows

No additional system dependencies required. The Rust toolchain includes everything needed.

## Build and Run

Clone the repository and run:

```bash
cargo run -p air-sim
```

For an optimised build:

```bash
cargo run -p air-sim --release
```

## Running Tests

```bash
cargo test -p sim-core
```
