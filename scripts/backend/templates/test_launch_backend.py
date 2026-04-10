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
        stop_event = mock.Mock()
        stop_event.wait.side_effect = [False, True]

        with mock.patch.object(
            launch_backend,
            "write_startup_heartbeat",
            side_effect=[False, False],
        ) as write_mock:
            launch_backend.heartbeat_loop(Path("/tmp/heartbeat.json"), 2.0, stop_event)

        self.assertEqual(
            [call.kwargs["warn_on_error"] for call in write_mock.call_args_list],
            [True, True],
        )

    def test_repeated_failures_after_success_are_suppressed(self) -> None:
        stop_event = mock.Mock()
        stop_event.wait.side_effect = [False, False, True]

        with mock.patch.object(
            launch_backend,
            "write_startup_heartbeat",
            side_effect=[True, False, False],
        ) as write_mock:
            launch_backend.heartbeat_loop(Path("/tmp/heartbeat.json"), 2.0, stop_event)

        self.assertEqual(
            [call.kwargs["warn_on_error"] for call in write_mock.call_args_list],
            [True, True, False],
        )

    def test_stop_failure_still_warns_after_earlier_failure(self) -> None:
        stop_event = mock.Mock()
        thread = mock.Mock()
        register = mock.Mock()

        with mock.patch.object(
            launch_backend,
            "write_startup_heartbeat",
            return_value=False,
        ) as write_mock:
            with mock.patch.object(
                launch_backend,
                "resolve_startup_heartbeat_path",
                return_value=Path("/tmp/heartbeat.json"),
            ):
                with mock.patch.object(
                    launch_backend.threading, "Event", return_value=stop_event
                ):
                    with mock.patch.object(
                        launch_backend.threading, "Thread", return_value=thread
                    ):
                        with mock.patch.object(
                            launch_backend.atexit, "register", register
                        ):
                            launch_backend.start_startup_heartbeat()
                            on_exit = register.call_args.args[0]
                            on_exit()

        self.assertEqual(
            [call.args[1] for call in write_mock.call_args_list],
            ["stopping"],
        )
        self.assertEqual(
            [call.kwargs["warn_on_error"] for call in write_mock.call_args_list],
            [True],
        )


if __name__ == "__main__":
    unittest.main()
