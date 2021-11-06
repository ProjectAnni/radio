# anni-radio

`anni-radio` is an audio streaming client for Project Anni.

## Dependency

- `ffmpeg`: You need `ffmpeg` in your `PATH`.

## Usage

```bash
# environmental variables required
ANNI_REPO=/path/to/repo
ANNI_RADIO_ROOT=/path/to/local/music/backend

# Use case 1:
# Stream to stdout and play with mpv
radio - | mpv -

# Use case 2:
# Stream to srt server
radio 'srt://xxx'
```
