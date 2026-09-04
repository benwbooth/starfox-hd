import unittest

from semantic_evidence import validate_object_inventory


class SemanticEvidenceTests(unittest.TestCase):
    def scene(self, active="0,1", free=None):
        if free is None:
            free = ",".join(str(slot) for slot in range(2, 70))
        return {"active_order": active, "free_order": free}

    def objects(self, slots=(0, 1), scene_id=1):
        return {(scene_id, slot): {"kind": "object"} for slot in slots}

    def valid(self):
        return {1: self.scene()}, self.objects()

    def assert_invalid(self, scenes, objects):
        with self.assertRaises(RuntimeError):
            validate_object_inventory("test", scenes, objects)

    def test_valid_complete_pool(self):
        scenes, objects = self.valid()
        validate_object_inventory("test", scenes, objects)

    def test_zero_active_allowed_with_all_free(self):
        scenes = {1: self.scene(active="", free=",".join(str(slot) for slot in range(70)))}
        validate_object_inventory("test", scenes, {})

    def test_absent_object(self):
        scenes, objects = self.valid()
        del objects[(1, 1)]
        self.assert_invalid(scenes, objects)

    def test_extra_object(self):
        scenes, objects = self.valid()
        objects[(1, 2)] = {"kind": "object"}
        self.assert_invalid(scenes, objects)

    def test_missing_lists(self):
        scenes, objects = self.valid()
        del scenes[1]["free_order"]
        self.assert_invalid(scenes, objects)

    def test_repeated_slot(self):
        scenes, objects = self.valid()
        scenes[1]["active_order"] = "0,0"
        self.assert_invalid(scenes, objects)

    def test_overlap(self):
        scenes, objects = self.valid()
        scenes[1]["free_order"] = "1," + ",".join(str(slot) for slot in range(2, 70))
        self.assert_invalid(scenes, objects)

    def test_incomplete_partition(self):
        scenes, objects = self.valid()
        scenes[1]["free_order"] = ",".join(str(slot) for slot in range(2, 69))
        self.assert_invalid(scenes, objects)

    def test_out_of_range(self):
        scenes = {1: self.scene(active="70", free=",".join(str(slot) for slot in range(69)))}
        self.assert_invalid(scenes, {})

    def test_unknown_scene_object(self):
        scenes, _ = self.valid()
        self.assert_invalid(scenes, {(2, 0): {"kind": "object"}})


if __name__ == "__main__":
    unittest.main()
