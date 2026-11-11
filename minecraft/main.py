import math
import random
from typing import Dict, Optional, Tuple

import pygame


WIDTH, HEIGHT = 1100, 700
FPS = 60
WORLD_SIZE = 24
TILE_WIDTH = 48
TILE_HEIGHT = 24
BLOCK_HEIGHT = 24

BlockPos = Tuple[int, int, int]
World = Dict[BlockPos, str]

BLOCK_COLORS = {
    "grass": ((93, 170, 72), (71, 132, 54), (55, 104, 44)),
    "dirt": ((151, 102, 57), (113, 73, 42), (87, 56, 35)),
    "stone": ((145, 148, 153), (108, 111, 116), (82, 84, 88)),
    "wood": ((180, 125, 67), (136, 87, 43), (100, 62, 34)),
}
BLOCK_NAMES = ["grass", "dirt", "stone", "wood"]


def generate_world() -> World:
    random.seed(7)
    world: World = {}
    for x in range(WORLD_SIZE):
        for y in range(WORLD_SIZE):
            height = 2 + int(
                1.7 * math.sin(x * 0.55) + 1.4 * math.cos(y * 0.42)
            )
            height = max(1, min(6, height))
            for z in range(height):
                if z == height - 1:
                    block = "grass"
                elif z >= height - 3:
                    block = "dirt"
                else:
                    block = "stone"
                world[(x, y, z)] = block

            if random.random() < 0.035 and height >= 3:
                for trunk_z in range(height, height + 3):
                    world[(x, y, trunk_z)] = "wood"
                for leaf_x in range(x - 1, x + 2):
                    for leaf_y in range(y - 1, y + 2):
                        if 0 <= leaf_x < WORLD_SIZE and 0 <= leaf_y < WORLD_SIZE:
                            world[(leaf_x, leaf_y, height + 3)] = "grass"
    return world


def project(pos: BlockPos, camera: pygame.Vector2) -> Tuple[float, float]:
    x, y, z = pos
    return (
        (x - y) * TILE_WIDTH / 2 + camera.x,
        (x + y) * TILE_HEIGHT / 2 - z * BLOCK_HEIGHT + camera.y,
    )


def diamond(center: Tuple[float, float], width: float, height: float):
    cx, cy = center
    return [
        (cx, cy),
        (cx + width / 2, cy + height / 2),
        (cx, cy + height),
        (cx - width / 2, cy + height / 2),
    ]


def block_polygons(pos: BlockPos, camera: pygame.Vector2):
    top_x, top_y = project(pos, camera)
    top = diamond((top_x, top_y), TILE_WIDTH, TILE_HEIGHT)
    left = [top[3], top[2], (top[2][0], top[2][1] + BLOCK_HEIGHT), (top[3][0], top[3][1] + BLOCK_HEIGHT)]
    right = [top[1], top[2], (top[2][0], top[2][1] + BLOCK_HEIGHT), (top[1][0], top[1][1] + BLOCK_HEIGHT)]
    return top, left, right


def visible_faces(pos: BlockPos, world: World):
    x, y, z = pos
    return (
        (x, y, z + 1) not in world,
        (x - 1, y, z) not in world,
        (x, y - 1, z) not in world,
    )


def draw_world(screen: pygame.Surface, world: World, camera: pygame.Vector2):
    ordered = sorted(world.items(), key=lambda item: (sum(item[0]), item[0][2]))
    for pos, block_type in ordered:
        top, left, right = block_polygons(pos, camera)
        colors = BLOCK_COLORS[block_type]
        show_top, show_left, show_right = visible_faces(pos, world)
        if show_left:
            pygame.draw.polygon(screen, colors[1], left)
        if show_right:
            pygame.draw.polygon(screen, colors[2], right)
        if show_top:
            pygame.draw.polygon(screen, colors[0], top)
            pygame.draw.lines(screen, (35, 55, 35), True, top, 1)


def block_at_mouse(
    mouse_pos: Tuple[int, int], world: World, camera: pygame.Vector2
) -> Optional[BlockPos]:
    candidates = []
    for pos in world:
        top, left, right = block_polygons(pos, camera)
        polygons = [top, left, right]
        for polygon in polygons:
            if pygame.Rect(
                min(point[0] for point in polygon),
                min(point[1] for point in polygon),
                max(point[0] for point in polygon) - min(point[0] for point in polygon) + 1,
                max(point[1] for point in polygon) - min(point[1] for point in polygon) + 1,
            ).collidepoint(mouse_pos) and point_in_polygon(mouse_pos, polygon):
                candidates.append(pos)
                break
    return max(candidates, key=lambda pos: (pos[0] + pos[1], pos[2])) if candidates else None


def point_in_polygon(point, polygon) -> bool:
    x, y = point
    inside = False
    previous = polygon[-1]
    for current in polygon:
        x1, y1 = current
        x2, y2 = previous
        if (y1 > y) != (y2 > y) and x < (x2 - x1) * (y - y1) / (y2 - y1) + x1:
            inside = not inside
        previous = current
    return inside


def draw_ui(screen: pygame.Surface, font: pygame.font.Font, selected: str):
    panel = pygame.Surface((WIDTH, 76), pygame.SRCALPHA)
    panel.fill((18, 24, 28, 220))
    screen.blit(panel, (0, 0))
    title = font.render("VoxelCraft prototype", True, (235, 240, 235))
    help_text = font.render(
        "WASD / arrows: move camera    Left click: mine    Right click: place    1-4: block",
        True,
        (195, 205, 195),
    )
    screen.blit(title, (18, 12))
    screen.blit(help_text, (18, 42))
    for index, name in enumerate(BLOCK_NAMES):
        rect = pygame.Rect(WIDTH - 230 + index * 52, 18, 42, 42)
        pygame.draw.rect(screen, BLOCK_COLORS[name][0], rect)
        pygame.draw.rect(screen, (255, 255, 255) if name == selected else (70, 80, 80), rect, 3)
        label = font.render(str(index + 1), True, (20, 25, 20))
        screen.blit(label, (rect.x + 16, rect.y + 12))


def main():
    pygame.init()
    screen = pygame.display.set_mode((WIDTH, HEIGHT))
    pygame.display.set_caption("VoxelCraft - Pygame prototype")
    clock = pygame.time.Clock()
    font = pygame.font.Font(None, 22)
    world = generate_world()
    camera = pygame.Vector2(WIDTH / 2, 180)
    selected = "grass"
    running = True

    while running:
        dt = clock.tick(FPS) / 1000
        for event in pygame.event.get():
            if event.type == pygame.QUIT:
                running = False
            elif event.type == pygame.KEYDOWN:
                if pygame.K_1 <= event.key <= pygame.K_4:
                    selected = BLOCK_NAMES[event.key - pygame.K_1]
            elif event.type == pygame.MOUSEBUTTONDOWN:
                target = block_at_mouse(event.pos, world, camera)
                if target is not None and event.button == 1:
                    del world[target]
                elif target is not None and event.button == 3:
                    x, y, z = target
                    placement = (x, y, z + 1)
                    if placement not in world:
                        world[placement] = selected

        keys = pygame.key.get_pressed()
        movement = pygame.Vector2(
            int(keys[pygame.K_RIGHT] or keys[pygame.K_d])
            - int(keys[pygame.K_LEFT] or keys[pygame.K_a]),
            int(keys[pygame.K_DOWN] or keys[pygame.K_s])
            - int(keys[pygame.K_UP] or keys[pygame.K_w]),
        )
        if movement.length_squared():
            camera += movement.normalize() * 280 * dt

        screen.fill((137, 195, 224))
        pygame.draw.rect(screen, (90, 155, 190), (0, HEIGHT * 0.65, WIDTH, HEIGHT * 0.35))
        draw_world(screen, world, camera)
        draw_ui(screen, font, selected)
        pygame.display.flip()

    pygame.quit()


if __name__ == "__main__":
    main()
