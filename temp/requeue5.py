import importlib, sys, pathlib, time
sys.path.insert(0, r'c:\Defi')
m = importlib.import_module('Defi_구현')
store = m.SQLiteStore()
slugs = [
    'modeb-ethereum-193-715185cd7f63',
    'modeb-ethereum-258-9a1d00ac6834',
    'modeb-clone-avalanche-258-9a1d00ac6834-9a1d001bd7dc',
    'modeb-clone-avalanche-001-f5b3164a485a-d1a1c2c8c1bc',
    'modeb-clone-avalanche-020-02411cd6e758-75c762634b1f',
]
with store._write_tx():
    for s in slugs:
        store._conn.execute("UPDATE audit_queue SET status='pending', claim_boot_id='', updated_at=? WHERE slug=?", (time.time(), s))
print(store.queue_counts())
