#!/usr/bin/env python3
"""Verify the actual 32-bit CLI in an offline Docker PTY, not a library harness."""
import argparse
import errno
import fcntl
import hashlib
import json
import os
from pathlib import Path
import pty
import select
import struct
import subprocess
import termios
import time

parser = argparse.ArgumentParser()
parser.add_argument('artifact', type=Path)
parser.add_argument('check', type=Path)
args = parser.parse_args()
out = args.artifact.resolve()
check = args.check.resolve()
assert check.name == 'ish-check' and (check / 'manifest.json').is_file()
assert (out / 'build.exit').read_text().strip() == '0'
assert (out / 'verification.exit').read_text().strip() == '0'
# Never replace the record of an earlier failed or successful PTY attempt.
assert not (out / 'ui-verification.log').exists(), 'Preserve earlier attempt before retrying'
name = (out / 'container-name.txt').read_text().strip() + '-ui-' + time.strftime('%H%M%S', time.gmtime())
image = (out / 'verify-image.txt').read_text().strip()
command = [
    'docker', 'run', '-it', '--platform', 'linux/386', '--network', 'none',
    '--read-only', '--tmpfs', '/tmp:rw,nosuid,nodev', '--name', name,
    '--label', 'tela.ish.owner=' + name, '--cidfile', str(out / 'ui-container-id.txt'),
    '--env', 'TERM=xterm-256color', '--env', 'HOME=/tmp', '--env', 'QEMU_CPU=pentium',
    '--mount', f'type=bind,src={out},dst=/artifact,readonly',
    '--mount', f'type=bind,src={check},dst=/check,readonly',
    image, '/artifact/tela', 'run', '/check',
]
(out / 'ui-container-name.txt').write_text(name + '\n')
(out / 'ui-command.json').write_text(json.dumps(command, indent=2) + '\n')
def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()
inputs = {'tela': digest(out / 'tela')}
inputs.update({str(p.relative_to(check)): digest(p) for p in sorted(check.rglob('*')) if p.is_file()})
(out / 'ui-input-sha256.json').write_text(json.dumps(inputs, indent=2) + '\n')
pid, fd = pty.fork()
if pid == 0:
    fcntl.ioctl(0, termios.TIOCSWINSZ, struct.pack('HHHH', 30, 100, 0, 0))
    os.execvp(command[0], command)
transcript = bytearray()
sent_quit = False
status = None
deadline = time.monotonic() + 180
try:
    with (out / 'ui-verification.log').open('wb') as log:
        while time.monotonic() < deadline:
            ready, _, _ = select.select([fd], [], [], 0.2)
            if ready:
                try:
                    data = os.read(fd, 65536)
                except OSError as exc:
                    if exc.errno != errno.EIO:
                        raise
                    data = b''
                if data:
                    log.write(data)
                    log.flush()
                    transcript.extend(data)
                    if not sent_quit and b'PASS: QuickJS/UI' in transcript:
                        os.write(fd, b'q')
                        sent_quit = True
            done, status_value = os.waitpid(pid, os.WNOHANG)
            if done:
                status = status_value
                break
finally:
    os.close(fd)
    if status is None:
        # Stop only our labeled verifier, never other workloads.
        owner = subprocess.check_output(['docker', 'inspect', '--format', '{{index .Config.Labels "tela.ish.owner"}}', name], text=True).strip()
        assert owner == name
        subprocess.run(['docker', 'stop', name], check=True, stdout=subprocess.DEVNULL)
        _, status = os.waitpid(pid, 0)
state = json.loads(subprocess.check_output(['docker', 'inspect', name]))[0]
(out / 'ui-container-state.json').write_text(json.dumps(state, indent=2) + '\n')
rc = os.waitstatus_to_exitcode(status)
passed = sent_quit and rc == 0 and state['State']['ExitCode'] == 0 and not state['State']['Running']
passed = passed and inputs['tela'] == digest(out / 'tela')
passed = passed and all(inputs[str(p.relative_to(check))] == digest(p) for p in check.rglob('*') if p.is_file())
(out / 'ui-verification.exit').write_text('0\n' if passed else '1\n')
(out / 'ui-result.json').write_text(json.dumps({'pass_marker': sent_quit, 'quit_sent': sent_quit, 'docker_client_exit': rc, 'container_exit': state['State']['ExitCode'], 'terminal': '100x30', 'passed': passed}, indent=2) + '\n')
print(f'PTY verification passed={passed}, client exit={rc}, container exit={state["State"]["ExitCode"]}; {out}/ui-verification.log')
raise SystemExit(0 if passed else 1)
