# Dicrs

**Dicrs** is a simple and efficient dictionary application written in Rust.
![Alt text](screenshot.png?raw=true "dicrs")
---

### Leitner System:
- Enable the Leitner review system to learn and review words effectively.
- Key bindings:
  - `Y`: Mark word as "correct".
  - `N`: Mark word as "incorrect".
  - `Enter` or `Space`: Show the definition of the selected word.
  - `Alt + L`: Switch to Default Mode.
  - `Alt + M`: Switch to Compact Mode.
  - `Up/Down Arrows`: Navigate the word index.

### Compact Mode:
- Minimal user interface to focus on essential functionality.
- Toggle compact mode with `Alt + M`.

---

## Installation

1. Clone the repository:
   ```bash
   git clone https://github.com/s2sadeghi/dicrs.git
   cd dicrs
   ```

2. Build the project using Cargo:
   ```bash
   cargo build --release
    ```

3. add your dictionaries to
    .local/share/dicrs/dictionaries

4. Run the binary:
   ```bash
   ./target/release/dicrs
   ```

---

## Usage

### Navigation:
- **Search Terms:**
  - Type the term and press `Enter` to search.
- **Navigate Results:**
  - `Up/Down Arrows`: Move through search results.
  - `Shift + Up/Down Arrows`: Jump 10 entries.

### Switching Modes:
- `Alt + L`: Switch to Leitner Mode.
- `Alt + M`: Toggle Compact Mode.

### Managing Leitner Entries:
- `~ (\`)`: Add the current word and its definition to Leitner.

---

## Configuration

Enable or disable features by using Cargo features:
- **Leitner Mode:**
  - Default: Enabled.
  - Disable: Add `--no-default-features` when building.

```bash
cargo build --release --no-default-features
```

---

## Key Bindings

| Key Combination      | Action                                     |
|----------------------|-------------------------------------------|
| `Ctrl + C`           | Exit application                          |
| `Ctrl + Y`           | Copy current definition to clipboard      |
| `Alt + L`            | Switch to Leitner Mode                    |
| `Alt + M`            | Toggle Compact Mode                       |
| `Up/Down Arrows`     | Navigate entries                          |
| `Shift + Up/Down`    | Jump 10 entries                           |
| `Left/Right Arrows`  | Switch between databases                  |
| `Backspace`          | Delete last character in the search input |
| Any Character        | Add character to the search input         |

---

## License

This project is licensed under the terms of the [GPL-3.0 License](LICENSE).

