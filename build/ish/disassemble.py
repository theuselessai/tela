#!/usr/bin/env python3
"""Inspect decoded instructions, not raw byte patterns or whole-ISA claims."""
import argparse
from collections import deque
import gzip
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess

parser = argparse.ArgumentParser()
parser.add_argument('artifact', type=Path)
args = parser.parse_args()
out = args.artifact.resolve()
tool = os.environ.get('LLVM_OBJDUMP') or shutil.which('llvm-objdump')
if not tool:
    tool = subprocess.check_output(['xcrun', '--find', 'llvm-objdump'], text=True).strip()
report = {'tool': tool, 'version': subprocess.check_output([tool, '--version'], text=True),
          'scope': 'Decoded LOCK NOT and zero-OR notification paths; NOT a whole-CPU ISA certification.',
          'artifacts': {}}
for name in ('tela', 'ish-smoke'):
    path = out / name
    digest = hashlib.sha256(path.read_bytes()).hexdigest()
    symbol = ''
    previous_lock = False
    context = deque(maxlen=8)
    nots = []
    ors = []
    with gzip.open(out / (name + '-disassembly.txt.gz'), 'wt') as saved:
        proc = subprocess.Popen([tool, '-d', '--demangle', str(path)], stdout=subprocess.PIPE, text=True)
        for line in proc.stdout:
            saved.write(line)
            if re.match(r'^[0-9a-f]+ <.*>:', line):
                symbol = line.strip()
            # Apple's LLVM prints LOCK on its own line. Also accept combined form.
            fields = line.strip().split('\t')
            instruction = ' '.join(fields[1:]).strip() if len(fields) > 1 else ''
            combined = instruction.startswith('lock ')
            operation = instruction[5:] if combined else instruction
            if previous_lock or combined:
                record = {'symbol': symbol, 'instruction': line.strip(), 'context': ''.join(context) + line}
                if re.match(r'not[lwbq]?\b', operation):
                    nots.append(record)
                if re.match(r'or[lwbq]?\s+\$0x?0\b', operation) or re.match(r'or[lwbq]?\s+\$0\b', operation):
                    if 'async_lock' in symbol or 'event_listener' in symbol:
                        ors.append(record)
            previous_lock = instruction == 'lock'
            context.append(line)
        assert proc.wait() == 0
    assert hashlib.sha256(path.read_bytes()).hexdigest() == digest
    report['artifacts'][name] = {'sha256': digest, 'lock_not': nots, 'notification_lock_or_zero': ors,
                                 'passed': not nots and bool(ors)}
(out / 'disassembly-review.json').write_text(json.dumps(report, indent=2) + '\n')
passed = all(item['passed'] for item in report['artifacts'].values())
(out / 'disassembly-verification.exit').write_text('0\n' if passed else '1\n')
for name, item in report['artifacts'].items():
    print(f"{name}: LOCK NOT={len(item['lock_not'])}, notification LOCK OR zero={len(item['notification_lock_or_zero'])}, passed={item['passed']}")
raise SystemExit(0 if passed else 1)
