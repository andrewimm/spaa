# Heap-Diff Format Specification

**Version 0.1**

Heap-diff is an NDJSON format for analyzing memory leaks by comparing two Chrome heap snapshots. It pre-computes the information an agent needs to identify leaked objects and understand why they weren't garbage collected.

## Purpose

When debugging memory leaks in JavaScript/Node.js applications, developers take heap snapshots before and after the suspected leak. Heap-diff processes these snapshots and produces an agent-friendly summary showing:

1. **What grew** - Object types with increased count or size
2. **What's retained** - New objects that weren't garbage collected
3. **Why it's retained** - The reference chain keeping each object alive

## File Format

- **Encoding**: UTF-8
- **Container**: NDJSON (newline-delimited JSON)
- **Ordering**: Header first, then growth records (sorted by size_delta descending), then retained records

Each line is a JSON object with a `type` field indicating the record type.

## Record Types

### Header (exactly one, must be first)

```json
{
  "type": "header",
  "format": "heap-diff",
  "version": "0.1",
  "baseline": "before.heapsnapshot",
  "target": "after.heapsnapshot"
}
```

| Field | Description |
|-------|-------------|
| `format` | Always `"heap-diff"` |
| `version` | Format version |
| `baseline` | Path to the baseline snapshot (before the leak) |
| `target` | Path to the target snapshot (after the leak) |

### Growth Records

Growth records summarize object types that increased between snapshots. They are sorted by `size_delta` descending. Types that didn't grow are omitted.

```json
{
  "type": "growth",
  "constructor": "Array",
  "count_before": 15000,
  "count_after": 16500,
  "count_delta": 1500,
  "size_before": 1200000,
  "size_after": 1445000,
  "size_delta": 245000
}
```

| Field | Description |
|-------|-------------|
| `constructor` | Object type/constructor name (e.g., `"Array"`, `"Object"`, `"MyClass"`) |
| `count_before` | Number of instances in baseline snapshot |
| `count_after` | Number of instances in target snapshot |
| `count_delta` | `count_after - count_before` |
| `size_before` | Total bytes in baseline snapshot |
| `size_after` | Total bytes in target snapshot |
| `size_delta` | `size_after - size_before` |

**Interpreting growth records:**
- Large `size_delta` indicates where memory is going
- Large `count_delta` with small `size_delta` suggests many small objects
- Focus on unexpected growth - some growth is normal during operation

### Retained Records

Retained records represent individual objects that exist in the target snapshot but not in the baseline (newly allocated and not garbage collected). These are sampled from the top growing types.

```json
{
  "type": "retained",
  "constructor": "Object",
  "size": 1024,
  "retention_path": ["Window", "app", "cache", "items", "[42]"]
}
```

| Field | Description |
|-------|-------------|
| `constructor` | Object type/constructor name |
| `size` | Size of this object in bytes |
| `retention_path` | Array of property names from GC root to this object |

**Interpreting retention paths:**

The `retention_path` shows the reference chain keeping an object alive, read left-to-right from GC root to the leaked object:

```
["Window", "app", "cache", "items", "[42]"]
```

Means: `Window.app.cache.items[42]` holds a reference to this object.

Common patterns:
- `["Window", "eventListeners", ...]` - Event listener not removed
- `["Window", "timers", ...]` - setInterval/setTimeout holding references
- `[..., "cache", ...]` - Unbounded cache growth
- `[..., "(closure)", ...]` - Closure capturing variables

**Note:** Retention paths may be truncated with `"..."` if very long (>20 segments), or empty if no path to a GC root was found within the search limit.

## Analysis Workflow

1. **Identify suspects**: Look at growth records with largest `size_delta`
2. **Examine retention**: Find retained records matching those constructors
3. **Trace the root cause**: Follow retention paths to understand why objects aren't freed
4. **Common fixes**:
   - Remove event listeners in cleanup/unmount
   - Clear intervals/timeouts
   - Add cache eviction policies
   - Break circular references
   - Avoid capturing large objects in closures

## Example Output

```json
{"type":"header","format":"heap-diff","version":"0.1","baseline":"snap1.heapsnapshot","target":"snap2.heapsnapshot"}
{"type":"growth","constructor":"Object","count_before":50000,"count_after":85000,"count_delta":35000,"size_before":4000000,"size_after":6800000,"size_delta":2800000}
{"type":"growth","constructor":"Array","count_before":12000,"count_after":13500,"count_delta":1500,"size_before":960000,"size_after":1080000,"size_delta":120000}
{"type":"growth","constructor":"IncomingMessage","count_before":0,"count_after":500,"count_delta":500,"size_before":0,"size_after":80000,"size_delta":80000}
{"type":"retained","constructor":"Object","size":80,"retention_path":["Window","server","_connections","[127]"]}
{"type":"retained","constructor":"IncomingMessage","size":160,"retention_path":["Window","server","_connections","[127]","request"]}
```

This example shows:
- 35,000 new Objects (~2.8MB) - largest growth
- 1,500 new Arrays (~120KB)
- 500 new IncomingMessage objects (~80KB) - likely HTTP requests
- Retention paths suggest `server._connections` is holding references to old requests
