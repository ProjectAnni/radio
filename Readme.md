# anni-radio

`anni-radio` is an audio streaming client for Project Anni.

## Installation

```bash
cargo install --git https://github.com/ProjectAnni/radio
```

## Dependency

- `ffmpeg`: You need `ffmpeg` in your `PATH`.

## Usage

```bash
# environmental variables required
ANNI_REPO=/path/to/repo
ANNIL_URL=https://annil-url/path
ANNIL_AUTH=eyxxxxxxx.yyyyyyy.zzzzzz

# Use case 1:
# Stream to stdout and play with mpv
radio - | mpv -

# Use case 2:
# Stream to srt server
radio 'srt://xxx'
```
