import time

def bounce_sim(steps):
    x = 0.0
    y = 0.0
    vx = 1.5
    vy = 2.3
    box_size = 100.0
    bounces = 0

    i = 0
    while i < steps:
        x += vx
        y += vy

        if x < 0.0:
            x = -x
            vx = -vx
            bounces += 1
        if x > box_size:
            x = box_size - (x - box_size)
            vx = -vx
            bounces += 1
        if y < 0.0:
            y = -y
            vy = -vy
            bounces += 1
        if y > box_size:
            y = box_size - (y - box_size)
            vy = -vy
            bounces += 1
        i += 1
    return bounces

steps = 10000000
start = time.monotonic_ns()
bounces = bounce_sim(steps)
elapsed = time.monotonic_ns() - start
ms = elapsed // 1000000
print(f"bounces: {bounces}")
print(f"elapsed: {ms} ms")
