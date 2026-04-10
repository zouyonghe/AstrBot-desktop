import importlib.util
import tempfile
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
    def test_atomic_write_json_cleans_up_temp_file_when_replace_fails(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            heartbeat_path = Path(temp_dir) / "heartbeat.json"
            temp_path = heartbeat_path.with_name(f"{heartbeat_path.name}.tmp")

            with mock.patch.object(
                Path,
                "replace",
                autospec=True,
                side_effect=OSError("replace failed"),
            ):
                with self.assertRaises(OSError):
                    launch_backend.atomic_write_json(
                        heartbeat_path,
                        {"pid": 42, "state": "starting", "updated_at_ms": 5000},
                    )

            self.assertFalse(temp_path.exists())

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
                            thread.join.assert_not_called()
                            on_exit = register.call_args.args[0]
                            on_exit()

        thread.join.assert_called_once_with(
            timeout=launch_backend.STARTUP_HEARTBEAT_STOP_JOIN_TIMEOUT_SECONDS
        )
        self.assertEqual(
            [call.args[1] for call in write_mock.call_args_list],
            ["stopping"],
        )
        self.assertEqual(
            [call.kwargs["warn_on_error"] for call in write_mock.call_args_list],
            [True],
        )

    def test_start_startup_heartbeat_does_not_register_exit_handler_when_thread_start_fails(
        self,
    ) -> None:
        stop_event = mock.Mock()
        thread = mock.Mock()
        thread.start.side_effect = RuntimeError("thread start failed")
        register = mock.Mock()

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
                    with mock.patch.object(launch_backend.atexit, "register", register):
                        with self.assertRaises(RuntimeError):
                            launch_backend.start_startup_heartbeat()

        register.assert_not_called()


if __name__ == "__main__":
    unittest.main()
