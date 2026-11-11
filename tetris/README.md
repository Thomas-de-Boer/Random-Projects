# Pygame Tetris

A polished desktop Tetris game built with Python and Pygame.

## Run

```text
pip install -r requirements.txt
python tetris.py
```

## Controls

| Key | Action |
|---|---|
| Left / Right | Move piece |
| Down | Soft drop |
| Up / X | Rotate clockwise |
| Z | Rotate counter-clockwise |
| Space | Hard drop |
| C | Hold piece |
| P | Pause |
| B | Toggle autoplay bot |
| R | Restart |
| Q | Quit |

The game uses the standard seven-piece randomizer, a five-piece queue, hold storage, line clears, combos, hard-drop scoring, and level progression every ten lines. The presentation includes a layered neon background, block highlights, landing ghost, glow effects, particles, screen feedback, animated clear flashes, score banners, hold/queue previews, and polished pause/game-over overlays. Press **B** to enable or disable the autoplay bot; it evaluates every legal rotation and position using height, holes, bumpiness, wells, and line clears.
