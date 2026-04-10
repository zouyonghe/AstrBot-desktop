import importlib.util
import unittest
from pathlib import Path
from unittest import mock


MODULE_PATH = Path(__file__).with_name("launch_backend.py")
SPEC = importlib.util.spec_from_file_location("launch_backend_under_test", MODULE_PATH)
if SPEC is None or SPEC.loader is None:
    raise RuntimeError(f"Cannot load launch_backend module from {MODULE_PATH}")
launch_backend = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(launch_backend)


class StartupHeartbeatTests(unittest.TestCase):
    def test_repeated_failures_warn_before_first_success(self) -> None:
        heartbeat = launch_backend.StartupHeartbeat(Path("/tmp/heartbeat.json"), 2.0)

        with mock.patch.object(
            launch_backend,
            "write_startup_heartbeat",
            side_effect=[False, False],
        ) as write_mock:
            heartbeat._write("starting", warn_on_error=True)
            heartbeat._write("starting", warn_on_error=True)

        self.assertEqual(
            [call.kwargs["warn_on_error"] for call in write_mock.call_args_list],
            [True, True],
        )

    def test_repeated_failures_after_success_are_suppressed(self) -> None:
        heartbeat = launch_backend.StartupHeartbeat(Path("/tmp/heartbeat.json"), 2.0)

        with mock.patch.object(
            launch_backend,
            "write_startup_heartbeat",
            side_effect=[True, False, False],
        ) as write_mock:
            heartbeat._write("starting", warn_on_error=True)
            heartbeat._write("starting", warn_on_error=True)
            heartbeat._write("starting", warn_on_error=True)

        self.assertEqual(
            [call.kwargs["warn_on_error"] for call in write_mock.call_args_list],
            [True, True, False],
        )

    def test_stop_failure_still_warns_after_earlier_failure(self) -> None:
        heartbeat = launch_backend.StartupHeartbeat(Path("/tmp/heartbeat.json"), 2.0)

        with mock.patch.object(
            launch_backend,
            "write_startup_heartbeat",
            side_effect=[False, False],
        ) as write_mock:
            heartbeat._write("starting", warn_on_error=True)
            heartbeat.stop()

        self.assertEqual(
            [call.args[1] for call in write_mock.call_args_list],
            ["starting", "stopping"],
        )
        self.assertEqual(
            [call.kwargs["warn_on_error"] for call in write_mock.call_args_list],
            [True, True],
        )


if __name__ == "__main__":
    unittest.main()
