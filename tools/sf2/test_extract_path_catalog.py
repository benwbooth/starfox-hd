"""Static path-catalog emission remains distinct from reviewed native roots."""
import importlib.util
from pathlib import Path
from dataclasses import replace
import unittest

spec = importlib.util.spec_from_file_location('path_catalog_emitter', Path(__file__).with_name('extract_path.py'))
emitter = importlib.util.module_from_spec(spec)
spec.loader.exec_module(emitter)


class PathCatalogEmissionTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        from rom import load_rom
        cls.extraction = emitter.PathExtractor(load_rom()).extract()

    def test_checked_in_catalog_matches_static_extraction(self):
        self.assertEqual((Path(emitter.RUST_SRC) / 'path.rs').read_text(), emitter.render_rust(self.extraction))

    def test_reviewed_but_unreachable_semantics_do_not_become_scanned_handlers(self):
        source = emitter.render_rust(self.extraction)
        unreachable = [s for s in emitter.PATH_SEMANTICS if s.opcode not in self.extraction.handlers]
        self.assertTrue(unreachable)
        for semantic in unreachable:
            self.assertNotIn(f'    {semantic.rust_name},', source)
            self.assertNotIn(f'Some(PathSemantic::{semantic.rust_name})', source)

    def test_changed_reachable_handler_or_ambiguous_graph_is_rejected(self):
        handlers = dict(self.extraction.handlers)
        opcode = next(iter(handlers))
        handlers[opcode] = replace(handlers[opcode], handler_address=handlers[opcode].handler_address + 1)
        with self.assertRaisesRegex(RuntimeError, 'no longer matches'):
            emitter.render_rust(replace(self.extraction, handlers=handlers))
        with self.assertRaisesRegex(RuntimeError, 'ambiguous path data'):
            emitter.render_rust(replace(self.extraction, unresolved_handlers=[opcode]))


if __name__ == '__main__':
    unittest.main()
