## 2026-01-30T22:55:00
- niveau: erreur
- contexte: codex_exec
- message: [red]Codex introuvable.[/red] codex missing
- exception: FileNotFoundError: codex missing
```
Traceback (most recent call last):
  File "C:\Users\nodig\PycharmProject\UsbDEV\usbide\app.py", line 388, in _run_codex
    async for ev in stream_subprocess(argv, cwd=self.root_dir, env=env):
    ...<24 lines>...
            self._codex_log_output(line)
  File "C:\Users\nodig\PycharmProject\UsbDEV\tests\test_app.py", line 173, in fake_stream
    raise FileNotFoundError("codex missing")
FileNotFoundError: codex missing
```
## 2026-01-30T22:59:55
- niveau: erreur
- contexte: codex_exec
- message: [red]Codex introuvable.[/red] [WinError 2] Le fichier spécifié est introuvable
- exception: FileNotFoundError: [WinError 2] Le fichier spécifié est introuvable
```
Traceback (most recent call last):
  File "C:\Users\nodig\PycharmProject\UsbDEV\usbide\app.py", line 388, in _run_codex
    async for ev in stream_subprocess(argv, cwd=self.root_dir, env=env):
    ...<24 lines>...
            self._codex_log_output(line)
  File "C:\Users\nodig\PycharmProject\UsbDEV\usbide\runner.py", line 38, in stream_subprocess
    proc = await asyncio.create_subprocess_exec(
           ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
    ...<5 lines>...
    )
    ^
  File "C:\Program Files\WindowsApps\PythonSoftwareFoundation.Python.3.13_3.13.2544.0_x64__qbz5n2kfra8p0\Lib\asyncio\subprocess.py", line 224, in create_subprocess_exec
    transport, protocol = await loop.subprocess_exec(
                          ^^^^^^^^^^^^^^^^^^^^^^^^^^^
    ...<3 lines>...
        stderr=stderr, **kwds)
        ^^^^^^^^^^^^^^^^^^^^^^
  File "C:\Program Files\WindowsApps\PythonSoftwareFoundation.Python.3.13_3.13.2544.0_x64__qbz5n2kfra8p0\Lib\asyncio\base_events.py", line 1802, in subprocess_exec
    transport = await self._make_subprocess_transport(
                ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
        protocol, popen_args, False, stdin, stdout, stderr,
        ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
        bufsize, **kwargs)
        ^^^^^^^^^^^^^^^^^^
  File "C:\Program Files\WindowsApps\PythonSoftwareFoundation.Python.3.13_3.13.2544.0_x64__qbz5n2kfra8p0\Lib\asyncio\windows_events.py", line 401, in _make_subprocess_transport
    transp = _WindowsSubprocessTransport(self, protocol, args, shell,
                                         stdin, stdout, stderr, bufsize,
                                         waiter=waiter, extra=extra,
                                         **kwargs)
  File "C:\Program Files\WindowsApps\PythonSoftwareFoundation.Python.3.13_3.13.2544.0_x64__qbz5n2kfra8p0\Lib\asyncio\base_subprocess.py", line 39, in __init__
    self._start(args=args, shell=shell, stdin=stdin, stdout=stdout,
    ~~~~~~~~~~~^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
                stderr=stderr, bufsize=bufsize, **kwargs)
                ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
  File "C:\Program Files\WindowsApps\PythonSoftwareFoundation.Python.3.13_3.13.2544.0_x64__qbz5n2kfra8p0\Lib\asyncio\windows_events.py", line 879, in _start
    self._proc = windows_utils.Popen(
                 ~~~~~~~~~~~~~~~~~~~^
        args, shell=shell, stdin=stdin, stdout=stdout, stderr=stderr,
        ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
        bufsize=bufsize, **kwargs)
        ^^^^^^^^^^^^^^^^^^^^^^^^^^
  File "C:\Program Files\WindowsApps\PythonSoftwareFoundation.Python.3.13_3.13.2544.0_x64__qbz5n2kfra8p0\Lib\asyncio\windows_utils.py", line 153, in __init__
    super().__init__(args, stdin=stdin_rfd, stdout=stdout_wfd,
    ~~~~~~~~~~~~~~~~^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
                     stderr=stderr_wfd, **kwds)
                     ^^^^^^^^^^^^^^^^^^^^^^^^^^
  File "C:\Program Files\WindowsApps\PythonSoftwareFoundation.Python.3.13_3.13.2544.0_x64__qbz5n2kfra8p0\Lib\subprocess.py", line 1039, in __init__
    self._execute_child(args, executable, preexec_fn, close_fds,
    ~~~~~~~~~~~~~~~~~~~^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
                        pass_fds, cwd, env,
                        ^^^^^^^^^^^^^^^^^^^
    ...<5 lines>...
                        gid, gids, uid, umask,
                        ^^^^^^^^^^^^^^^^^^^^^^
                        start_new_session, process_group)
                        ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
  File "C:\Program Files\WindowsApps\PythonSoftwareFoundation.Python.3.13_3.13.2544.0_x64__qbz5n2kfra8p0\Lib\subprocess.py", line 1554, in _execute_child
    hp, ht, pid, tid = _winapi.CreateProcess(executable, args,
                       ~~~~~~~~~~~~~~~~~~~~~^^^^^^^^^^^^^^^^^^
                             # no special security
                             ^^^^^^^^^^^^^^^^^^^^^
    ...<4 lines>...
                             cwd,
                             ^^^^
                             startupinfo)
                             ^^^^^^^^^^^^
FileNotFoundError: [WinError 2] Le fichier spécifié est introuvable
```
## 2026-01-30T23:09:05
- niveau: erreur
- contexte: codex_exec
- message: [red]Codex introuvable.[/red] [WinError 2] Le fichier spécifié est introuvable
- exception: FileNotFoundError: [WinError 2] Le fichier spécifié est introuvable
```
Traceback (most recent call last):
  File "C:\Users\nodig\PycharmProject\UsbDEV\usbide\app.py", line 388, in _run_codex
    async for ev in stream_subprocess(argv, cwd=self.root_dir, env=env):
    ...<24 lines>...
            self._codex_log_output(line)
  File "C:\Users\nodig\PycharmProject\UsbDEV\usbide\runner.py", line 38, in stream_subprocess
    proc = await asyncio.create_subprocess_exec(
           ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
    ...<5 lines>...
    )
    ^
  File "C:\Program Files\WindowsApps\PythonSoftwareFoundation.Python.3.13_3.13.2544.0_x64__qbz5n2kfra8p0\Lib\asyncio\subprocess.py", line 224, in create_subprocess_exec
    transport, protocol = await loop.subprocess_exec(
                          ^^^^^^^^^^^^^^^^^^^^^^^^^^^
    ...<3 lines>...
        stderr=stderr, **kwds)
        ^^^^^^^^^^^^^^^^^^^^^^
  File "C:\Program Files\WindowsApps\PythonSoftwareFoundation.Python.3.13_3.13.2544.0_x64__qbz5n2kfra8p0\Lib\asyncio\base_events.py", line 1802, in subprocess_exec
    transport = await self._make_subprocess_transport(
                ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
        protocol, popen_args, False, stdin, stdout, stderr,
        ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
        bufsize, **kwargs)
        ^^^^^^^^^^^^^^^^^^
  File "C:\Program Files\WindowsApps\PythonSoftwareFoundation.Python.3.13_3.13.2544.0_x64__qbz5n2kfra8p0\Lib\asyncio\windows_events.py", line 401, in _make_subprocess_transport
    transp = _WindowsSubprocessTransport(self, protocol, args, shell,
                                         stdin, stdout, stderr, bufsize,
                                         waiter=waiter, extra=extra,
                                         **kwargs)
  File "C:\Program Files\WindowsApps\PythonSoftwareFoundation.Python.3.13_3.13.2544.0_x64__qbz5n2kfra8p0\Lib\asyncio\base_subprocess.py", line 39, in __init__
    self._start(args=args, shell=shell, stdin=stdin, stdout=stdout,
    ~~~~~~~~~~~^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
                stderr=stderr, bufsize=bufsize, **kwargs)
                ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
  File "C:\Program Files\WindowsApps\PythonSoftwareFoundation.Python.3.13_3.13.2544.0_x64__qbz5n2kfra8p0\Lib\asyncio\windows_events.py", line 879, in _start
    self._proc = windows_utils.Popen(
                 ~~~~~~~~~~~~~~~~~~~^
        args, shell=shell, stdin=stdin, stdout=stdout, stderr=stderr,
        ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
        bufsize=bufsize, **kwargs)
        ^^^^^^^^^^^^^^^^^^^^^^^^^^
  File "C:\Program Files\WindowsApps\PythonSoftwareFoundation.Python.3.13_3.13.2544.0_x64__qbz5n2kfra8p0\Lib\asyncio\windows_utils.py", line 153, in __init__
    super().__init__(args, stdin=stdin_rfd, stdout=stdout_wfd,
    ~~~~~~~~~~~~~~~~^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
                     stderr=stderr_wfd, **kwds)
                     ^^^^^^^^^^^^^^^^^^^^^^^^^^
  File "C:\Program Files\WindowsApps\PythonSoftwareFoundation.Python.3.13_3.13.2544.0_x64__qbz5n2kfra8p0\Lib\subprocess.py", line 1039, in __init__
    self._execute_child(args, executable, preexec_fn, close_fds,
    ~~~~~~~~~~~~~~~~~~~^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
                        pass_fds, cwd, env,
                        ^^^^^^^^^^^^^^^^^^^
    ...<5 lines>...
                        gid, gids, uid, umask,
                        ^^^^^^^^^^^^^^^^^^^^^^
                        start_new_session, process_group)
                        ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
  File "C:\Program Files\WindowsApps\PythonSoftwareFoundation.Python.3.13_3.13.2544.0_x64__qbz5n2kfra8p0\Lib\subprocess.py", line 1554, in _execute_child
    hp, ht, pid, tid = _winapi.CreateProcess(executable, args,
                       ~~~~~~~~~~~~~~~~~~~~~^^^^^^^^^^^^^^^^^^
                             # no special security
                             ^^^^^^^^^^^^^^^^^^^^^
    ...<4 lines>...
                             cwd,
                             ^^^^
                             startupinfo)
                             ^^^^^^^^^^^^
FileNotFoundError: [WinError 2] Le fichier spécifié est introuvable
```
## 2026-01-30T23:55:26
- niveau: erreur
- contexte: codex_exec
- message: [red]Codex introuvable.[/red] codex missing
- exception: FileNotFoundError: codex missing
```
Traceback (most recent call last):
  File "C:\Users\nodig\PycharmProject\UsbDEV\usbide\app.py", line 391, in _run_codex
    async for ev in stream_subprocess(argv, cwd=self.root_dir, env=env):
    ...<24 lines>...
            self._codex_log_output(line)
  File "C:\Users\nodig\PycharmProject\UsbDEV\tests\test_app.py", line 173, in fake_stream
    raise FileNotFoundError("codex missing")
FileNotFoundError: codex missing
```
## 2026-01-30T23:56:00
- niveau: erreur
- contexte: codex_exec
- message: [red]Codex introuvable.[/red] codex missing
- exception: FileNotFoundError: codex missing
```
Traceback (most recent call last):
  File "C:\Users\nodig\PycharmProject\UsbDEV\usbide\app.py", line 391, in _run_codex
    async for ev in stream_subprocess(argv, cwd=self.root_dir, env=env):
    ...<24 lines>...
            self._codex_log_output(line)
  File "C:\Users\nodig\PycharmProject\UsbDEV\tests\test_app.py", line 173, in fake_stream
    raise FileNotFoundError("codex missing")
FileNotFoundError: codex missing
```
## 2026-01-31T00:07:02
- niveau: erreur
- contexte: codex_exec
- message: [red]Codex termine en erreur (rc=1).[/red]
## 2026-01-31T00:50:01
- niveau: erreur
- contexte: codex_exec
- message: [red]Codex introuvable.[/red] codex missing
- exception: FileNotFoundError: codex missing
```
Traceback (most recent call last):
  File "C:\Users\nodig\PycharmProject\UsbDEV\usbide\app.py", line 469, in _run_codex
    async for ev in stream_subprocess(argv, cwd=self.root_dir, env=env):
    ...<59 lines>...
            self._codex_log_output(json.dumps(obj, ensure_ascii=False))
  File "C:\Users\nodig\PycharmProject\UsbDEV\tests\test_app.py", line 232, in fake_stream
    raise FileNotFoundError("codex missing")
FileNotFoundError: codex missing
```
## 2026-01-31T01:09:52
- niveau: erreur
- contexte: codex_exec
- message: [red]Codex introuvable.[/red] codex missing
- exception: FileNotFoundError: codex missing
```
Traceback (most recent call last):
  File "C:\Users\nodig\PycharmProject\UsbDEV\usbide\app.py", line 469, in _run_codex
    async for ev in stream_subprocess(argv, cwd=self.root_dir, env=env):
    ...<59 lines>...
            self._codex_log_output(json.dumps(obj, ensure_ascii=False))
  File "C:\Users\nodig\PycharmProject\UsbDEV\tests\test_app.py", line 232, in fake_stream
    raise FileNotFoundError("codex missing")
FileNotFoundError: codex missing
```
## 2026-01-31T01:45:13
- niveau: erreur
- contexte: codex_exec
- message: [red]Codex introuvable.[/red] codex missing
- exception: FileNotFoundError: codex missing
```
Traceback (most recent call last):
  File "C:\Users\nodig\PycharmProject\UsbDEV\usbide\app.py", line 570, in _run_codex
    async for ev in stream_subprocess(argv, cwd=self.root_dir, env=env):
    ...<68 lines>...
            self._codex_log_output(json.dumps(obj, ensure_ascii=False))
  File "C:\Users\nodig\PycharmProject\UsbDEV\tests\test_app.py", line 272, in fake_stream
    raise FileNotFoundError("codex missing")
FileNotFoundError: codex missing
```
## 2026-01-31T01:45:31
- niveau: erreur
- contexte: codex_exec
- message: [red]Codex introuvable.[/red] codex missing
- exception: FileNotFoundError: codex missing
```
Traceback (most recent call last):
  File "C:\Users\nodig\PycharmProject\UsbDEV\usbide\app.py", line 570, in _run_codex
    async for ev in stream_subprocess(argv, cwd=self.root_dir, env=env):
    ...<68 lines>...
            self._codex_log_output(json.dumps(obj, ensure_ascii=False))
  File "C:\Users\nodig\PycharmProject\UsbDEV\tests\test_app.py", line 272, in fake_stream
    raise FileNotFoundError("codex missing")
FileNotFoundError: codex missing
```
## 2026-01-31T01:45:55
- niveau: erreur
- contexte: codex_exec
- message: [red]Codex introuvable.[/red] codex missing
- exception: FileNotFoundError: codex missing
```
Traceback (most recent call last):
  File "C:\Users\nodig\PycharmProject\UsbDEV\usbide\app.py", line 570, in _run_codex
    async for ev in stream_subprocess(argv, cwd=self.root_dir, env=env):
    ...<68 lines>...
            self._codex_log_output(json.dumps(obj, ensure_ascii=False))
  File "C:\Users\nodig\PycharmProject\UsbDEV\tests\test_app.py", line 272, in fake_stream
    raise FileNotFoundError("codex missing")
FileNotFoundError: codex missing
```
## 2026-01-31T01:59:47
- niveau: erreur
- contexte: codex_exec
- message: [red]Codex introuvable.[/red] codex missing
- exception: FileNotFoundError: codex missing
```
Traceback (most recent call last):
  File "C:\Users\nodig\PycharmProject\UsbDEV\usbide\app.py", line 570, in _run_codex
    async for ev in stream_subprocess(argv, cwd=self.root_dir, env=env):
    ...<68 lines>...
            self._codex_log_output(json.dumps(obj, ensure_ascii=False))
  File "C:\Users\nodig\PycharmProject\UsbDEV\tests\test_app.py", line 272, in fake_stream
    raise FileNotFoundError("codex missing")
FileNotFoundError: codex missing
```
## 2026-01-31T02:02:51
- niveau: erreur
- contexte: codex_exec
- message: [red]Codex introuvable.[/red] codex missing
- exception: FileNotFoundError: codex missing
```
Traceback (most recent call last):
  File "C:\Users\nodig\PycharmProject\UsbDEV\usbide\app.py", line 570, in _run_codex
    async for ev in stream_subprocess(argv, cwd=self.root_dir, env=env):
    ...<68 lines>...
            self._codex_log_output(json.dumps(obj, ensure_ascii=False))
  File "C:\Users\nodig\PycharmProject\UsbDEV\tests\test_app.py", line 359, in fake_stream
    raise FileNotFoundError("codex missing")
FileNotFoundError: codex missing
```
## 2026-01-31T02:03:09
- niveau: erreur
- contexte: codex_exec
- message: [red]Codex introuvable.[/red] codex missing
- exception: FileNotFoundError: codex missing
```
Traceback (most recent call last):
  File "C:\Users\nodig\PycharmProject\UsbDEV\usbide\app.py", line 570, in _run_codex
    async for ev in stream_subprocess(argv, cwd=self.root_dir, env=env):
    ...<68 lines>...
            self._codex_log_output(json.dumps(obj, ensure_ascii=False))
  File "C:\Users\nodig\PycharmProject\UsbDEV\tests\test_app.py", line 362, in fake_stream
    raise FileNotFoundError("codex missing")
FileNotFoundError: codex missing
```
## 2026-01-31T10:03:50
- niveau: erreur
- contexte: codex_exec
- message: [red]Codex introuvable.[/red] codex missing
- exception: FileNotFoundError: codex missing
```
Traceback (most recent call last):
  File "C:\Users\nodig\PycharmProject\UsbDEV\usbide\app.py", line 738, in _run_codex
    async for ev in stream_subprocess(argv, cwd=self.root_dir, env=env):
    ...<105 lines>...
            self._codex_log_output(json.dumps(obj, ensure_ascii=False))
  File "C:\Users\nodig\PycharmProject\UsbDEV\tests\test_app.py", line 404, in fake_stream
    raise FileNotFoundError("codex missing")
FileNotFoundError: codex missing
```
## 2026-01-31T10:04:12
- niveau: erreur
- contexte: codex_exec
- message: [red]Codex introuvable.[/red] codex missing
- exception: FileNotFoundError: codex missing
```
Traceback (most recent call last):
  File "C:\Users\nodig\PycharmProject\UsbDEV\usbide\app.py", line 738, in _run_codex
    async for ev in stream_subprocess(argv, cwd=self.root_dir, env=env):
    ...<105 lines>...
            self._codex_log_output(json.dumps(obj, ensure_ascii=False))
  File "C:\Users\nodig\PycharmProject\UsbDEV\tests\test_app.py", line 404, in fake_stream
    raise FileNotFoundError("codex missing")
FileNotFoundError: codex missing
```
## 2026-01-31T17:43:02
- niveau: erreur
- contexte: codex_exec
- message: [red]Codex introuvable.[/red] codex missing
- exception: FileNotFoundError: codex missing
```
Traceback (most recent call last):
  File "C:\Users\nodig\PycharmProject\UsbDEV\usbide\app.py", line 782, in _run_codex
    async for ev in stream_subprocess(argv, cwd=self.root_dir, env=env):
    ...<105 lines>...
            self._codex_log_output(json.dumps(obj, ensure_ascii=False))
  File "C:\Users\nodig\PycharmProject\UsbDEV\tests\test_app.py", line 404, in fake_stream
    raise FileNotFoundError("codex missing")
FileNotFoundError: codex missing
```
## 2026-01-31T17:43:20
- niveau: erreur
- contexte: codex_exec
- message: [red]Codex introuvable.[/red] codex missing
- exception: FileNotFoundError: codex missing
```
Traceback (most recent call last):
  File "C:\Users\nodig\PycharmProject\UsbDEV\usbide\app.py", line 782, in _run_codex
    async for ev in stream_subprocess(argv, cwd=self.root_dir, env=env):
    ...<105 lines>...
            self._codex_log_output(json.dumps(obj, ensure_ascii=False))
  File "C:\Users\nodig\PycharmProject\UsbDEV\tests\test_app.py", line 404, in fake_stream
    raise FileNotFoundError("codex missing")
FileNotFoundError: codex missing
```
## 2026-01-31T17:50:18
- niveau: erreur
- contexte: codex_exec
- message: [red]Codex introuvable.[/red] codex missing
- exception: FileNotFoundError: codex missing
```
Traceback (most recent call last):
  File "C:\Users\nodig\PycharmProject\UsbDEV\usbide\app.py", line 813, in _run_codex
    async for ev in stream_subprocess(argv, cwd=self.root_dir, env=env):
    ...<105 lines>...
            self._codex_log_output(json.dumps(obj, ensure_ascii=False))
  File "C:\Users\nodig\PycharmProject\UsbDEV\tests\test_app.py", line 410, in fake_stream
    raise FileNotFoundError("codex missing")
FileNotFoundError: codex missing
```
## 2026-01-31T17:59:26
- niveau: erreur
- contexte: codex_exec
- message: [red]Codex introuvable.[/red] codex missing
- exception: FileNotFoundError: codex missing
```
Traceback (most recent call last):
  File "C:\Users\nodig\PycharmProject\UsbDEV\usbide\app.py", line 813, in _run_codex
    async for ev in stream_subprocess(argv, cwd=self.root_dir, env=env):
    ...<105 lines>...
            self._codex_log_output(json.dumps(obj, ensure_ascii=False))
  File "C:\Users\nodig\PycharmProject\UsbDEV\tests\test_app.py", line 410, in fake_stream
    raise FileNotFoundError("codex missing")
FileNotFoundError: codex missing
```
## 2026-01-31T18:05:35
- niveau: erreur
- contexte: codex_exec
- message: [red]Codex introuvable.[/red] codex missing
- exception: FileNotFoundError: codex missing
```
Traceback (most recent call last):
  File "C:\Users\nodig\PycharmProject\UsbDEV\usbide\app.py", line 816, in _run_codex
    async for ev in stream_subprocess(argv, cwd=self.root_dir, env=env):
    ...<105 lines>...
            self._codex_log_output(json.dumps(obj, ensure_ascii=False))
  File "C:\Users\nodig\PycharmProject\UsbDEV\tests\test_app.py", line 424, in fake_stream
    raise FileNotFoundError("codex missing")
FileNotFoundError: codex missing
```
## 2026-01-31T18:07:31
- niveau: erreur
- contexte: codex_exec
- message: [red]Erreur execution Codex:[/red] Separator is not found, and chunk exceed the limit
- exception: ValueError: Separator is not found, and chunk exceed the limit
```
Traceback (most recent call last):
  File "C:\Users\nodig\AppData\Local\Programs\Python\Python313\Lib\asyncio\streams.py", line 562, in readline
    line = await self.readuntil(sep)
           ^^^^^^^^^^^^^^^^^^^^^^^^^
  File "C:\Users\nodig\AppData\Local\Programs\Python\Python313\Lib\asyncio\streams.py", line 663, in readuntil
    raise exceptions.LimitOverrunError(
        'Separator is not found, and chunk exceed the limit',
        offset)
asyncio.exceptions.LimitOverrunError: Separator is not found, and chunk exceed the limit

During handling of the above exception, another exception occurred:

Traceback (most recent call last):
  File "C:\Users\nodig\PycharmProject\UsbDEV\usbide\app.py", line 813, in _run_codex
    # Robustesse: on capture les erreurs de lancement pour eviter un crash UI.
        ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
    ...<105 lines>...
            if isinstance(obj, dict) and isinstance(obj.get("type"), str):
            ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
  File "C:\Users\nodig\PycharmProject\UsbDEV\usbide\runner.py", line 56, in stream_subprocess
    raw = await proc.stdout.readline()
          ^^^^^^^^^^^^^^^^^^^^^^^^^^^^
  File "C:\Users\nodig\AppData\Local\Programs\Python\Python313\Lib\asyncio\streams.py", line 571, in readline
    raise ValueError(e.args[0])
ValueError: Separator is not found, and chunk exceed the limit
```
## 2026-01-31T18:13:46
- niveau: erreur
- contexte: codex_exec
- message: [red]Codex introuvable.[/red] codex missing
- exception: FileNotFoundError: codex missing
```
Traceback (most recent call last):
  File "C:\Users\nodig\PycharmProject\UsbDEV\usbide\app.py", line 868, in _run_codex
    async for ev in stream_subprocess(argv, cwd=self.root_dir, env=env):
    ...<105 lines>...
            self._codex_log_output(json.dumps(obj, ensure_ascii=False))
  File "C:\Users\nodig\PycharmProject\UsbDEV\tests\test_app.py", line 424, in fake_stream
    raise FileNotFoundError("codex missing")
FileNotFoundError: codex missing
```
## 2026-01-31T18:14:46
- niveau: erreur
- contexte: codex_exec
- message: [red]Codex introuvable.[/red] codex missing
- exception: FileNotFoundError: codex missing
```
Traceback (most recent call last):
  File "C:\Users\nodig\PycharmProject\UsbDEV\usbide\app.py", line 868, in _run_codex
    async for ev in stream_subprocess(argv, cwd=self.root_dir, env=env):
    ...<105 lines>...
            self._codex_log_output(json.dumps(obj, ensure_ascii=False))
  File "C:\Users\nodig\PycharmProject\UsbDEV\tests\test_app.py", line 424, in fake_stream
    raise FileNotFoundError("codex missing")
FileNotFoundError: codex missing
```
## 2026-01-31T18:15:04
- niveau: erreur
- contexte: codex_exec
- message: [red]Codex introuvable.[/red] codex missing
- exception: FileNotFoundError: codex missing
```
Traceback (most recent call last):
  File "C:\Users\nodig\PycharmProject\UsbDEV\usbide\app.py", line 868, in _run_codex
    async for ev in stream_subprocess(argv, cwd=self.root_dir, env=env):
    ...<105 lines>...
            self._codex_log_output(json.dumps(obj, ensure_ascii=False))
  File "C:\Users\nodig\PycharmProject\UsbDEV\tests\test_app.py", line 424, in fake_stream
    raise FileNotFoundError("codex missing")
FileNotFoundError: codex missing
```
## 2026-01-31T18:15:34
- niveau: erreur
- contexte: codex_exec
- message: [red]Codex introuvable.[/red] codex missing
- exception: FileNotFoundError: codex missing
```
Traceback (most recent call last):
  File "C:\Users\nodig\PycharmProject\UsbDEV\usbide\app.py", line 868, in _run_codex
    async for ev in stream_subprocess(argv, cwd=self.root_dir, env=env):
    ...<105 lines>...
            self._codex_log_output(json.dumps(obj, ensure_ascii=False))
  File "C:\Users\nodig\PycharmProject\UsbDEV\tests\test_app.py", line 424, in fake_stream
    raise FileNotFoundError("codex missing")
FileNotFoundError: codex missing
```
## 2026-01-31T18:26:03
- niveau: erreur
- contexte: codex_exec
- message: [red]Codex introuvable.[/red] codex missing
- exception: FileNotFoundError: codex missing
```
Traceback (most recent call last):
  File "C:\Users\nodig\PycharmProject\UsbDEV\usbide\app.py", line 868, in _run_codex
    async for ev in stream_subprocess(argv, cwd=self.root_dir, env=env):
    ...<105 lines>...
            self._codex_log_output(json.dumps(obj, ensure_ascii=False))
  File "C:\Users\nodig\PycharmProject\UsbDEV\tests\test_app.py", line 423, in fake_stream
    raise FileNotFoundError("codex missing")
FileNotFoundError: codex missing
```
