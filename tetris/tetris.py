"""A polished Pygame Tetris game.

Run with:
    python tetris.py

Controls:
    Left/Right  Move
    Down        Soft drop
    Up or X     Rotate clockwise
    Z           Rotate counter-clockwise
    Space       Hard drop
    P           Pause
    B           Toggle autoplay bot
    R           Restart
    Esc/Q       Quit
"""

from __future__ import annotations

import random
import sys
from dataclasses import dataclass
from typing import Iterable

import pygame


WIDTH, HEIGHT = 10, 20
CELL = 30
BOARD_RECT = pygame.Rect(40, 60, WIDTH * CELL, HEIGHT * CELL)
WINDOW_SIZE = (720, 720)
INPUT_REPEAT_DELAY = 0.18
INPUT_REPEAT_INTERVAL = 0.12
MAX_PARTICLES = 220

BG = (12, 15, 28)
PANEL = (23, 28, 48)
PANEL_LIGHT = (31, 38, 63)
GRID = (36, 43, 67)
TEXT = (235, 240, 255)
MUTED = (137, 148, 180)
COLORS = {
    "I": (52, 211, 235),
    "J": (77, 119, 237),
    "L": (247, 157, 55),
    "O": (246, 211, 70),
    "S": (72, 205, 126),
    "T": (190, 102, 235),
    "Z": (238, 83, 108),
}
ACCENT = (103, 232, 210)
GOLD = (255, 208, 92)

SHAPES: dict[str, tuple[tuple[int, int], ...]] = {
    "I": ((0, 1), (1, 1), (2, 1), (3, 1)),
    "J": ((0, 0), (0, 1), (1, 1), (2, 1)),
    "L": ((2, 0), (0, 1), (1, 1), (2, 1)),
    "O": ((1, 0), (2, 0), (1, 1), (2, 1)),
    "S": ((1, 0), (2, 0), (0, 1), (1, 1)),
    "T": ((1, 0), (0, 1), (1, 1), (2, 1)),
    "Z": ((0, 0), (1, 0), (1, 1), (2, 1)),
}


@dataclass
class Piece:
    kind: str
    x: int = 3
    y: int = 0
    rotation: int = 0

    def cells(self) -> tuple[tuple[int, int], ...]:
        cells = SHAPES[self.kind]
        if self.kind == "O":
            return cells
        rotated = cells
        for _ in range(self.rotation % 4):
            rotated = tuple((-y, x) for x, y in rotated)
        min_x = min(x for x, _ in rotated)
        min_y = min(y for _, y in rotated)
        return tuple((x - min_x, y - min_y) for x, y in rotated)

    def absolute_cells(self) -> Iterable[tuple[int, int]]:
        return ((self.x + x, self.y + y) for x, y in self.cells())


@dataclass
class Particle:
    x: float
    y: float
    vx: float
    vy: float
    color: tuple[int, int, int]
    life: float
    size: float

    def update(self, dt: float) -> bool:
        self.x += self.vx * dt
        self.y += self.vy * dt
        self.vy += 520 * dt
        self.life -= dt
        return self.life > 0


class Tetris:
    def __init__(self) -> None:
        self.reset()

    def reset(self) -> None:
        self.board: list[list[str | None]] = [[None] * WIDTH for _ in range(HEIGHT)]
        self.randomizer: list[str] = []
        self.queue: list[str] = []
        self.score = self.lines = 0
        self.level = 1
        self.combo = -1
        self.last_clear = 0
        self.hold_kind: str | None = None
        self.can_hold = True
        self.game_over = self.paused = False
        self.bot_enabled = False
        self.particles: list[Particle] = []
        self.line_flash = 0.0
        self.banner = ""
        self.banner_timer = 0.0
        self.shake = 0.0
        self.ensure_queue(5)
        self.next_kind = self.queue.pop(0)
        self.spawn()

    def ensure_queue(self, count: int) -> None:
        while len(self.queue) < count:
            if not self.randomizer:
                self.randomizer = list(SHAPES)
                random.shuffle(self.randomizer)
            self.queue.append(self.randomizer.pop())

    def take_piece_kind(self) -> str:
        if not self.randomizer:
            self.randomizer = list(SHAPES)
            random.shuffle(self.randomizer)
        return self.randomizer.pop()

    def spawn(self, reset_hold: bool = True) -> None:
        self.current = Piece(self.next_kind)
        self.ensure_queue(1)
        self.next_kind = self.queue.pop(0)
        self.ensure_queue(5)
        if reset_hold:
            self.can_hold = True
        if not self.valid(self.current):
            self.game_over = True

    def valid(self, piece: Piece) -> bool:
        for x, y in piece.absolute_cells():
            if x < 0 or x >= WIDTH or y >= HEIGHT:
                return False
            if y >= 0 and self.board[y][x] is not None:
                return False
        return True

    def move(self, dx: int, dy: int) -> bool:
        if self.game_over or self.paused:
            return False
        candidate = Piece(self.current.kind, self.current.x + dx,
                          self.current.y + dy, self.current.rotation)
        if self.valid(candidate):
            self.current = candidate
            return True
        return False

    def rotate(self, direction: int = 1) -> bool:
        if self.game_over or self.paused:
            return False
        rotation = (self.current.rotation + direction) % 4
        for kick in (0, -1, 1, -2, 2):
            candidate = Piece(self.current.kind, self.current.x + kick,
                              self.current.y, rotation)
            if self.valid(candidate):
                self.current = candidate
                return True
        return False

    def hard_drop(self) -> None:
        distance = 0
        while self.move(0, 1):
            distance += 1
        self.score += distance * 2
        self.emit_piece_particles(self.current, 8)
        self.shake = max(self.shake, 0.12)
        self.lock()

    def hold(self) -> None:
        if self.game_over or self.paused or not self.can_hold:
            return
        kind = self.current.kind
        self.can_hold = False
        if self.hold_kind is None:
            self.hold_kind = kind
            self.spawn(reset_hold=False)
            self.can_hold = False
        else:
            self.hold_kind, self.current = kind, Piece(self.hold_kind)
            if not self.valid(self.current):
                self.game_over = True
        self.banner = "HOLD"
        self.banner_timer = 0.5

    def tick(self) -> None:
        if not self.game_over and not self.paused and not self.move(0, 1):
            self.lock()

    def lock(self) -> None:
        locked_piece = self.current
        for x, y in self.current.absolute_cells():
            if y < 0:
                self.game_over = True
                return
            self.board[y][x] = self.current.kind
        cleared = self.clear_lines()
        self.lines += cleared
        self.level = self.lines // 10 + 1
        self.last_clear = cleared
        self.score += (0, 100, 300, 500, 800)[cleared] * self.level
        if cleared:
            self.combo += 1
            self.score += max(0, self.combo) * 50 * self.level
            self.banner = ("SINGLE", "DOUBLE", "TRIPLE", "TETRIS")[cleared - 1]
            if self.combo > 0:
                self.banner += f"  COMBO x{self.combo + 1}"
            self.banner_timer = 1.0
            self.shake = max(self.shake, 0.1 + cleared * 0.06)
        else:
            self.combo = -1
        self.emit_piece_particles(locked_piece, 10)
        self.spawn()

    def clear_lines(self) -> int:
        remaining = [row for row in self.board if any(cell is None for cell in row)]
        cleared = HEIGHT - len(remaining)
        self.board = [[None] * WIDTH for _ in range(cleared)] + remaining
        if cleared:
            self.line_flash = 0.24
            for y, row in enumerate(self.board):
                if y < cleared:
                    for x in range(WIDTH):
                        self.emit_particles_at(x, y, (255, 245, 180), 2)
        return cleared

    def emit_particles_at(self, x: int, y: int,
                          color: tuple[int, int, int], amount: int = 1) -> None:
        for _ in range(amount):
            if len(self.particles) >= MAX_PARTICLES:
                break
            self.particles.append(Particle(
                BOARD_RECT.x + x * CELL + CELL / 2,
                BOARD_RECT.y + y * CELL + CELL / 2,
                random.uniform(-110, 110), random.uniform(-180, -35),
                color, random.uniform(0.35, 0.75), random.uniform(2, 5)))

    def emit_piece_particles(self, piece: Piece, amount: int) -> None:
        for x, y in piece.absolute_cells():
            if 0 <= y < HEIGHT:
                self.emit_particles_at(x, y, COLORS[piece.kind], max(1, amount // 4))

    def update_effects(self, dt: float) -> None:
        self.particles = [particle for particle in self.particles
                          if particle.update(dt)]
        self.line_flash = max(0.0, self.line_flash - dt)
        self.banner_timer = max(0.0, self.banner_timer - dt)
        self.shake = max(0.0, self.shake - dt)

    def drop_interval(self) -> float:
        return max(0.08, 0.8 - (self.level - 1) * 0.06)

    def ghost(self) -> Piece:
        piece = Piece(self.current.kind, self.current.x, self.current.y,
                      self.current.rotation)
        while self.valid(Piece(piece.kind, piece.x, piece.y + 1, piece.rotation)):
            piece.y += 1
        return piece

    def bot_step(self) -> None:
        """Choose and place the safest move using a strong board heuristic."""
        if self.game_over or self.paused:
            return
        move = self.best_bot_move(self.current.kind, self.board)
        if move is None:
            self.game_over = True
            return
        rotation, x, y = move
        self.current = Piece(self.current.kind, x, y, rotation)
        self.lock()

    def best_bot_move(
        self, kind: str, board: list[list[str | None]]
    ) -> tuple[int, int, int] | None:
        best: tuple[float, tuple[int, int, int]] | None = None
        for rotation in range(4):
            shape = Piece(kind, 0, 0, rotation).cells()
            width = max(x for x, _ in shape) + 1
            for x in range(WIDTH - width + 1):
                y = 0
                candidate = Piece(kind, x, y, rotation)
                if any(cell_y < 0 for _, cell_y in candidate.absolute_cells()):
                    continue
                while self.valid_on_board(candidate, board):
                    y += 1
                    candidate.y = y
                candidate.y -= 1
                if candidate.y < 0 or not self.valid_on_board(candidate, board):
                    continue
                result = [row[:] for row in board]
                for cell_x, cell_y in candidate.absolute_cells():
                    result[cell_y][cell_x] = kind
                cleared = sum(all(cell is not None for cell in row) for row in result)
                result = [row for row in result if any(cell is None for cell in row)]
                result = [[None] * WIDTH for _ in range(cleared)] + result
                score = self.board_score(result, cleared)
                if best is None or score > best[0]:
                    best = (score, (rotation, x, candidate.y))
        return best[1] if best else None

    @staticmethod
    def valid_on_board(piece: Piece, board: list[list[str | None]]) -> bool:
        for x, y in piece.absolute_cells():
            if x < 0 or x >= WIDTH or y >= HEIGHT:
                return False
            if y >= 0 and board[y][x] is not None:
                return False
        return True

    @staticmethod
    def board_score(board: list[list[str | None]], cleared: int) -> float:
        heights: list[int] = []
        holes = 0
        for x in range(WIDTH):
            column_height = 0
            found_block = False
            for y in range(HEIGHT):
                if board[y][x] is not None:
                    if not found_block:
                        column_height = HEIGHT - y
                        found_block = True
                elif found_block:
                    holes += 1
            heights.append(column_height)
        aggregate_height = sum(heights)
        bumpiness = sum(abs(heights[x] - heights[x + 1]) for x in range(WIDTH - 1))
        max_height = max(heights, default=0)
        wells = sum(
            max(0, min(heights[x - 1:x + 2]) - heights[x])
            for x in range(1, WIDTH - 1)
        )
        return (
            cleared * 8.0
            - aggregate_height * 0.51
            - holes * 7.5
            - bumpiness * 0.35
            - max_height * 0.55
            + wells * 0.15
        )


def text(surface: pygame.Surface, font: pygame.font.Font, value: str,
         position: tuple[int, int], color: tuple[int, int, int] = TEXT) -> None:
    surface.blit(font.render(value, True, color), position)


def draw_block(surface: pygame.Surface, x: int, y: int, color: tuple[int, int, int],
               size: int = CELL, ghost: bool = False) -> None:
    rect = pygame.Rect(x, y, size, size)
    if ghost:
        pygame.draw.rect(surface, tuple(min(255, c + 18) for c in color), rect, 2, 5)
        return
    pygame.draw.rect(surface, color, rect.inflate(-3, -3), border_radius=5)
    highlight = tuple(min(255, c + 45) for c in color)
    pygame.draw.line(surface, highlight, (rect.x + 7, rect.y + 5),
                     (rect.right - 8, rect.y + 5), 2)
    pygame.draw.line(surface, highlight, (rect.x + 5, rect.y + 7),
                     (rect.x + 5, rect.bottom - 8), 2)


def draw_preview(surface: pygame.Surface, kind: str, rect: pygame.Rect) -> None:
    cells = SHAPES[kind]
    min_x = min(x for x, _ in cells)
    max_x = max(x for x, _ in cells)
    min_y = min(y for _, y in cells)
    max_y = max(y for _, y in cells)
    size = 24
    offset_x = rect.centerx - ((max_x - min_x + 1) * size) // 2
    offset_y = rect.centery - ((max_y - min_y + 1) * size) // 2
    for x, y in cells:
        draw_block(surface, offset_x + (x - min_x) * size,
                   offset_y + (y - min_y) * size, COLORS[kind], size)


def draw_glow(surface: pygame.Surface, center: tuple[int, int],
              color: tuple[int, int, int], radius: int = 60) -> None:
    glow = pygame.Surface((radius * 2, radius * 2), pygame.SRCALPHA)
    for size in range(radius, 4, -6):
        alpha = max(0, int(34 * (1 - size / radius)))
        pygame.draw.circle(glow, (*color, alpha), (radius, radius), size)
    surface.blit(glow, (center[0] - radius, center[1] - radius))


def draw(game: Tetris, surface: pygame.Surface, fonts: dict[str, pygame.font.Font]) -> None:
    surface.fill(BG)
    for y in range(WINDOW_SIZE[1]):
        shade = int(12 + 12 * y / WINDOW_SIZE[1])
        pygame.draw.line(surface, (shade, shade + 3, 28 + int(24 * y / WINDOW_SIZE[1])),
                         (0, y), (WINDOW_SIZE[0], y))
    title, heading, body, small = fonts["title"], fonts["heading"], fonts["body"], fonts["small"]
    text(surface, title, "TETRIS", (40, 18))
    text(surface, small, "CLASSIC BLOCK PUZZLE", (187, 31), MUTED)
    draw_glow(surface, (BOARD_RECT.centerx, BOARD_RECT.centery), (42, 90, 130), 190)

    pygame.draw.rect(surface, PANEL, BOARD_RECT.inflate(16, 16), border_radius=12)
    pygame.draw.rect(surface, (18, 22, 38), BOARD_RECT, border_radius=7)
    for y in range(HEIGHT):
        for x in range(WIDTH):
            cell = game.board[y][x]
            rect = pygame.Rect(BOARD_RECT.x + x * CELL, BOARD_RECT.y + y * CELL, CELL, CELL)
            pygame.draw.rect(surface, GRID, rect, 1)
            if cell:
                draw_block(surface, rect.x, rect.y, COLORS[cell])
    if game.line_flash:
        flash = pygame.Surface((BOARD_RECT.width, BOARD_RECT.height), pygame.SRCALPHA)
        alpha = int(210 * game.line_flash / 0.24)
        flash.fill((255, 246, 190, alpha))
        surface.blit(flash, BOARD_RECT.topleft)

    if not game.game_over:
        ghost = game.ghost()
        for x, y in ghost.absolute_cells():
            if 0 <= y < HEIGHT:
                draw_block(surface, BOARD_RECT.x + x * CELL, BOARD_RECT.y + y * CELL,
                           COLORS[ghost.kind], ghost=True)
        for x, y in game.current.absolute_cells():
            if 0 <= y < HEIGHT:
                draw_block(surface, BOARD_RECT.x + x * CELL, BOARD_RECT.y + y * CELL,
                           COLORS[game.current.kind])

    panel = pygame.Rect(390, 60, 290, 600)
    pygame.draw.rect(surface, PANEL, panel, border_radius=12)
    text(surface, heading, "HOLD", (415, 84))
    hold_box = pygame.Rect(415, 125, 112, 90)
    pygame.draw.rect(surface, PANEL_LIGHT, hold_box, border_radius=10)
    if game.hold_kind:
        draw_preview(surface, game.hold_kind, hold_box)
    text(surface, small, "READY" if game.can_hold else "USED", (466, 190),
         ACCENT if game.can_hold else MUTED)
    text(surface, heading, "NEXT", (545, 84))
    next_box = pygame.Rect(545, 125, 110, 90)
    pygame.draw.rect(surface, PANEL_LIGHT, next_box, border_radius=10)
    draw_preview(surface, game.next_kind, next_box)
    text(surface, small, "UP NEXT", (545, 222), MUTED)
    for index, kind in enumerate(game.queue[:3]):
        draw_preview(surface, kind, pygame.Rect(545, 242 + index * 42, 110, 38))

    for label, value, y in (("SCORE", game.score, 375), ("LINES", game.lines, 435),
                            ("LEVEL", game.level, 495)):
        text(surface, small, label, (420, y), MUTED)
        text(surface, heading, f"{value:,}", (420, y + 20))

    pygame.draw.line(surface, GRID, (415, 560), (655, 560), 1)
    text(surface, heading, "CONTROLS", (415, 575))
    controls = ("← →  Move", "↓  Soft drop", "↑ / X  Rotate", "Z  Rotate back",
                "SPACE  Hard drop", "C  Hold   P  Pause", "R  Restart   B  Bot")
    for index, control in enumerate(controls):
        text(surface, small, control, (420, 590 + index * 13),
             COLORS["S"] if game.bot_enabled and index == 6 else MUTED)
    if game.bot_enabled:
        text(surface, small, "BOT ACTIVE", (545, 31), COLORS["S"])

    for particle in game.particles:
        alpha = max(0, min(255, int(255 * min(1, particle.life / 0.25))))
        particle_surface = pygame.Surface((12, 12), pygame.SRCALPHA)
        pygame.draw.circle(particle_surface, (*particle.color, alpha),
                           (6, 6), max(1, int(particle.size)))
        surface.blit(particle_surface, (int(particle.x - 6), int(particle.y - 6)))

    if game.banner_timer:
        scale = 1.0 + max(0, 1.0 - game.banner_timer) * 0.08
        banner_font = pygame.font.SysFont("segoeui", int(28 * scale), bold=True)
        banner = banner_font.render(game.banner, True, GOLD)
        surface.blit(banner, (BOARD_RECT.centerx - banner.get_width() // 2, 85))

    if game.paused or game.game_over:
        overlay = pygame.Surface(surface.get_size(), pygame.SRCALPHA)
        overlay.fill((5, 7, 15, 185))
        surface.blit(overlay, (0, 0))
        message = "PAUSED" if game.paused else "GAME OVER"
        text(surface, title, message, (BOARD_RECT.centerx - 105, 300),
             COLORS["O"] if game.paused else COLORS["Z"])
        text(surface, body, "Press R to restart" if game.game_over
             else "Press P to continue", (BOARD_RECT.centerx - 78, 350), TEXT)


def run() -> None:
    pygame.init()
    pygame.display.set_caption("Tetris")
    surface = pygame.display.set_mode(WINDOW_SIZE)
    fonts = {
        "title": pygame.font.SysFont("segoeui", 38, bold=True),
        "heading": pygame.font.SysFont("segoeui", 22, bold=True),
        "body": pygame.font.SysFont("segoeui", 18),
        "small": pygame.font.SysFont("segoeui", 15),
    }
    game = Tetris()
    clock = pygame.time.Clock()
    fall_timer = 0.0
    bot_timer = 0.0
    input_timer = 0.0
    held_direction: int | None = None
    running = True
    while running:
        dt = clock.tick(60) / 1000
        fall_timer += dt
        for event in pygame.event.get():
            if event.type == pygame.QUIT:
                running = False
            elif event.type == pygame.KEYDOWN:
                if event.key in (pygame.K_ESCAPE, pygame.K_q):
                    running = False
                elif event.key == pygame.K_r:
                    game.reset()
                    fall_timer = bot_timer = 0
                elif event.key == pygame.K_b:
                    game.bot_enabled = not game.bot_enabled
                    bot_timer = 0
                elif event.key == pygame.K_c and not game.bot_enabled:
                    game.hold()
                elif event.key == pygame.K_p and not game.game_over:
                    game.paused = not game.paused
                elif event.key == pygame.K_LEFT and not game.bot_enabled:
                    game.move(-1, 0)
                    held_direction = pygame.K_LEFT
                    input_timer = -INPUT_REPEAT_DELAY
                elif event.key == pygame.K_RIGHT and not game.bot_enabled:
                    game.move(1, 0)
                    held_direction = pygame.K_RIGHT
                    input_timer = -INPUT_REPEAT_DELAY
                elif event.key == pygame.K_DOWN and not game.bot_enabled:
                    game.move(0, 1)
                elif event.key in (pygame.K_UP, pygame.K_x) and not game.bot_enabled:
                    game.rotate()
                elif event.key == pygame.K_z and not game.bot_enabled:
                    game.rotate(-1)
                elif event.key == pygame.K_SPACE and not game.bot_enabled:
                    game.hard_drop()
                    fall_timer = 0
            elif event.type == pygame.KEYUP:
                if event.key in (pygame.K_LEFT, pygame.K_RIGHT):
                    if held_direction == event.key:
                        held_direction = None
                        input_timer = 0

        if game.bot_enabled:
            held_direction = None
            input_timer = 0
        elif held_direction is not None:
            input_timer += dt
            if input_timer >= INPUT_REPEAT_INTERVAL:
                if held_direction == pygame.K_LEFT:
                    game.move(-1, 0)
                elif held_direction == pygame.K_RIGHT:
                    game.move(1, 0)
                input_timer -= INPUT_REPEAT_INTERVAL

        if game.bot_enabled:
            bot_timer += dt
            if bot_timer >= 0.08:
                game.bot_step()
                bot_timer = 0
        elif fall_timer >= game.drop_interval():
            game.tick()
            fall_timer = 0
        game.update_effects(dt)
        draw(game, surface, fonts)
        pygame.display.flip()
    pygame.quit()
    sys.exit()


if __name__ == "__main__":
    run()
