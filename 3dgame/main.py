"""Neon Maze: a tiny dependency-free 3D console game.

Run with:
    python main.py

Controls:
    W/S  move forward/backward
    A/D  turn left/right
    Q/E  strafe left/right
    SPACE shoot
    R    restart after winning or losing
    ESC  quit
"""

from __future__ import annotations

import math
import os
import random
import shutil
import sys
import time
from dataclasses import dataclass


MAP = [
    "####################",
    "#........#.........#",
    "#.######.#.#######.#",
    "#.#....#.#.......#.#",
    "#.#.##.#.#######.#.#",
    "#...##.#.....#...#.#",
    "###.##.#####.#.###.#",
    "#...#......#.#.....#",
    "#.####.###.#.#####.#",
    "#......#...#.......#",
    "#.######.#########.#",
    "#.................X#",
    "####################",
]
MAP_W = len(MAP[0])
MAP_H = len(MAP)


@dataclass
class Enemy:
    x: float
    y: float
    health: int = 2
    cooldown: float = 0.0


@dataclass
class Player:
    x: float = 2.5
    y: float = 1.5
    angle: float = 0.0
    health: int = 100
    ammo: int = 18
    score: int = 0


def clear_screen() -> None:
    sys.stdout.write("\x1b[2J\x1b[H")


def cell(x: float, y: float) -> str:
    ix, iy = int(x), int(y)
    if iy < 0 or iy >= MAP_H or ix < 0 or ix >= MAP_W:
        return "#"
    return MAP[iy][ix]


def blocked(x: float, y: float) -> bool:
    return cell(x, y) == "#"


def move_player(player: Player, dx: float, dy: float) -> None:
    # Separate axes prevent the player from snagging on wall corners.
    if not blocked(player.x + dx, player.y):
        player.x += dx
    if not blocked(player.x, player.y + dy):
        player.y += dy


def cast_ray(px: float, py: float, angle: float, max_depth: float = 20.0) -> float:
    step = 0.035
    distance = 0.0
    while distance < max_depth:
        distance += step
        if blocked(px + math.cos(angle) * distance, py + math.sin(angle) * distance):
            return distance
    return max_depth


def line_of_sight(ax: float, ay: float, bx: float, by: float) -> bool:
    distance = math.hypot(bx - ax, by - ay)
    steps = max(1, int(distance / 0.08))
    for i in range(1, steps):
        t = i / steps
        if blocked(ax + (bx - ax) * t, ay + (by - ay) * t):
            return False
    return True


def make_enemies() -> list[Enemy]:
    return [
        Enemy(7.5, 1.5),
        Enemy(16.5, 3.5),
        Enemy(5.5, 7.5),
        Enemy(13.5, 9.5),
        Enemy(17.5, 11.5),
    ]


def shoot(player: Player, enemies: list[Enemy]) -> str:
    if player.ammo <= 0:
        return "CLICK... out of ammo"
    player.ammo -= 1
    best: tuple[float, Enemy] | None = None
    for enemy in enemies:
        dx, dy = enemy.x - player.x, enemy.y - player.y
        distance = math.hypot(dx, dy)
        relative = math.atan2(math.sin(math.atan2(dy, dx) - player.angle),
                               math.cos(math.atan2(dy, dx) - player.angle))
        if abs(relative) < 0.09 and line_of_sight(player.x, player.y, enemy.x, enemy.y):
            if best is None or distance < best[0]:
                best = (distance, enemy)
    if best is None:
        return "BANG! The shot hits the darkness"
    enemy = best[1]
    enemy.health -= 1
    if enemy.health <= 0:
        enemies.remove(enemy)
        player.score += 100
        return "DIRECT HIT! Enemy destroyed"
    return "HIT! Enemy damaged"


def update_enemies(player: Player, enemies: list[Enemy], dt: float) -> str:
    message = ""
    for enemy in enemies:
        enemy.cooldown = max(0.0, enemy.cooldown - dt)
        dx, dy = player.x - enemy.x, player.y - enemy.y
        distance = math.hypot(dx, dy)
        if distance > 1.15 and line_of_sight(enemy.x, enemy.y, player.x, player.y):
            speed = 0.45 * dt
            nx, ny = enemy.x + dx / distance * speed, enemy.y + dy / distance * speed
            if not blocked(nx, enemy.y):
                enemy.x = nx
            if not blocked(enemy.x, ny):
                enemy.y = ny
        if distance <= 1.25 and enemy.cooldown <= 0:
            player.health -= 8
            enemy.cooldown = 1.0
            message = "WARNING: something claws at you!"
    return message


def render(player: Player, enemies: list[Enemy], message: str, width: int, height: int) -> str:
    view_w = max(50, min(width, 110))
    view_h = max(18, min(height - 5, 38))
    fov = math.pi / 3
    depth = [20.0] * view_w
    rows = [[" " for _ in range(view_w)] for _ in range(view_h)]
    shades = " .:-=+*#%@"

    for column in range(view_w):
        ray_angle = player.angle - fov / 2 + fov * column / view_w
        distance = cast_ray(player.x, player.y, ray_angle)
        corrected = max(0.01, distance * math.cos(ray_angle - player.angle))
        depth[column] = corrected
        wall_height = min(view_h, int(view_h / corrected))
        top = (view_h - wall_height) // 2
        shade = shades[min(len(shades) - 1, int((1.0 - corrected / 20.0) * len(shades)))]
        for row in range(view_h):
            if row < top:
                rows[row][column] = " "
            elif row < top + wall_height:
                rows[row][column] = shade
            else:
                rows[row][column] = "." if row > view_h * 0.65 else " "

    # Project enemies as billboard sprites, depth-tested against the walls.
    visible = []
    for enemy in enemies:
        dx, dy = enemy.x - player.x, enemy.y - player.y
        distance = math.hypot(dx, dy)
        relative = math.atan2(math.sin(math.atan2(dy, dx) - player.angle),
                              math.cos(math.atan2(dy, dx) - player.angle))
        if abs(relative) < fov / 2 and line_of_sight(player.x, player.y, enemy.x, enemy.y):
            visible.append((distance, relative, enemy))
    for distance, relative, enemy in sorted(visible, reverse=True):
        screen_x = int((0.5 + relative / fov) * view_w)
        size = max(1, min(view_h, int(view_h / max(0.4, distance))))
        left, right = screen_x - size // 3, screen_x + size // 3
        top = view_h // 2 - size // 2
        for x in range(left, right + 1):
            if 0 <= x < view_w and distance < depth[x]:
                for y in range(top, top + size):
                    if 0 <= y < view_h:
                        rows[y][x] = "M" if (x + y) % 3 else "W"

    crosshair = view_h // 2
    if 0 <= crosshair < view_h:
        rows[crosshair][view_w // 2] = "+"
    hud = f" HP {max(0, player.health):03d}   AMMO {player.ammo:02d}   SCORE {player.score:04d}   HOSTILES {len(enemies)} "
    controls = "W/S move  A/D turn  Q/E strafe  SPACE fire  ESC quit"
    output = ["+" + "-" * view_w + "+", "|" + hud[:view_w].ljust(view_w) + "|"]
    output += ["|" + "".join(row) + "|" for row in rows]
    output += ["|" + (message[:view_w].center(view_w) if message else " " * view_w) + "|",
               "|" + controls[:view_w].center(view_w) + "|",
               "+" + "-" * view_w + "+"]
    return "\n".join(output)


def title_screen(width: int) -> None:
    clear_screen()
    print("""
   ███╗   ██╗███████╗ ██████╗ ███╗   ██╗
   ████╗  ██║██╔════╝██╔═══██╗████╗  ██║
   ██╔██╗ ██║█████╗  ██║   ██║██╔██╗ ██║
   ██║╚██╗██║██╔══╝  ██║   ██║██║╚██╗██║
   ██║ ╚████║███████╗╚██████╔╝██║ ╚████║
   ╚═╝  ╚═══╝╚══════╝ ╚═════╝ ╚═╝  ╚═══╝
                 M A Z E
""")
    print("A tiny 3D console shooter. Find the exit after clearing the maze.")
    print("Press any key to start, or ESC to quit.")
    while True:
        if os.name == "nt":
            import msvcrt
            if msvcrt.kbhit():
                key = msvcrt.getwch()
                if key == "\x1b":
                    return False
                return True
        time.sleep(0.05)


def run() -> None:
    if os.name != "nt":
        print("This game currently targets a Windows terminal (it uses msvcrt for real-time input).")
        return
    import msvcrt

    width, height = shutil.get_terminal_size((90, 32))
    while True:
        if not title_screen(width):
            return
        player, enemies = Player(), make_enemies()
        message = "Clear the hostiles, then reach the green exit (X)."
        last = time.monotonic()
        playing = True
        while playing:
            now = time.monotonic()
            dt = min(0.1, now - last)
            last = now
            while msvcrt.kbhit():
                key = msvcrt.getwch().lower()
                if key == "\x1b":
                    return
                if key == "w":
                    move_player(player, math.cos(player.angle) * 0.22, math.sin(player.angle) * 0.22)
                elif key == "s":
                    move_player(player, -math.cos(player.angle) * 0.18, -math.sin(player.angle) * 0.18)
                elif key == "a":
                    player.angle -= 0.14
                elif key == "d":
                    player.angle += 0.14
                elif key == "q":
                    move_player(player, math.cos(player.angle - math.pi / 2) * 0.18,
                                 math.sin(player.angle - math.pi / 2) * 0.18)
                elif key == "e":
                    move_player(player, math.cos(player.angle + math.pi / 2) * 0.18,
                                 math.sin(player.angle + math.pi / 2) * 0.18)
                elif key == " ":
                    message = shoot(player, enemies)
            enemy_message = update_enemies(player, enemies, dt)
            if enemy_message:
                message = enemy_message
            if player.health <= 0:
                playing = False
                message = "YOU DIED - press R to restart or ESC to quit"
            elif not enemies and cell(player.x, player.y) == "X":
                playing = False
                message = "YOU ESCAPED THE NEON MAZE! press R to play again"
            clear_screen()
            print(render(player, enemies, message, width, height))
            if playing:
                time.sleep(0.035)
        while True:
            if msvcrt.kbhit():
                key = msvcrt.getwch().lower()
                if key == "\x1b":
                    return
                if key == "r":
                    break
            time.sleep(0.05)


if __name__ == "__main__":
    try:
        run()
    except KeyboardInterrupt:
        pass
    finally:
        clear_screen()
