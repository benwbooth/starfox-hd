"""Validation for bounded SF1 scene/object semantic evidence."""


def _parse_order(label, scene_id, field, value, object_count):
    """Parse one comma-separated slot order and return its slot set."""
    if not isinstance(value, str):
        raise RuntimeError(
            f"{label}: scene {scene_id} {field} must be a comma-separated string"
        )

    text = value.strip()
    if not text:
        return set()

    slots = []
    for token in text.split(","):
        token = token.strip()
        if not token or not token.isdigit():
            raise RuntimeError(
                f"{label}: scene {scene_id} {field} has invalid slot {token!r}"
            )
        slot = int(token)
        if slot < 0 or slot >= object_count:
            raise RuntimeError(
                f"{label}: scene {scene_id} {field} slot {slot} is outside 0..{object_count - 1}"
            )
        if slot in slots:
            raise RuntimeError(
                f"{label}: scene {scene_id} {field} repeats slot {slot}"
            )
        slots.append(slot)
    return set(slots)


def validate_object_inventory(
    label, scenes: dict[int, dict[str, str]], objects: dict[tuple[int, int], dict[str, str]], object_count: int = 70
) -> None:
    """Validate complete active/free slot and object-record evidence.

    Each scene's active and free orders must form a disjoint partition of the
    bounded slot pool. Object records are keyed by ``(scene_id, slot_id)`` and
    must exist exactly for the scene's active slots.
    """
    if not isinstance(object_count, int) or isinstance(object_count, bool) or object_count <= 0:
        raise RuntimeError(f"{label}: object_count must be a positive integer")

    for key in objects:
        if not isinstance(key, tuple) or len(key) != 2:
            raise RuntimeError(f"{label}: invalid object key {key!r}")
        scene_id, slot = key
        if scene_id not in scenes:
            raise RuntimeError(f"{label}: object {key!r} belongs to unknown scene")
        if not isinstance(slot, int) or isinstance(slot, bool):
            raise RuntimeError(f"{label}: object {key!r} has a non-integer slot")
        if slot < 0 or slot >= object_count:
            raise RuntimeError(
                f"{label}: object {key!r} slot is outside 0..{object_count - 1}"
            )

    pool = set(range(object_count))
    for scene_id, scene in scenes.items():
        if not isinstance(scene, dict):
            raise RuntimeError(f"{label}: scene {scene_id} must be a mapping")
        for field in ("active_order", "free_order"):
            if field not in scene:
                raise RuntimeError(f"{label}: scene {scene_id} is missing {field}")

        active = _parse_order(label, scene_id, "active_order", scene["active_order"], object_count)
        free = _parse_order(label, scene_id, "free_order", scene["free_order"], object_count)
        overlap = active & free
        if overlap:
            raise RuntimeError(
                f"{label}: scene {scene_id} active/free orders overlap at slots {sorted(overlap)}"
            )
        if active | free != pool:
            missing = sorted(pool - (active | free))
            raise RuntimeError(
                f"{label}: scene {scene_id} active/free orders do not partition the pool; missing {missing}"
            )

        recorded = {slot for scene_key, slot in objects if scene_key == scene_id}
        if recorded != active:
            missing = sorted(active - recorded)
            extra = sorted(recorded - active)
            raise RuntimeError(
                f"{label}: scene {scene_id} object records do not match active slots; "
                f"missing {missing}, extra {extra}"
            )
