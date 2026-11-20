+++
name = "Keys"

[params]
subject = "A figure's name, `camera` or `look`."
action = "`list`, `delete`, `shift` in time, set a `curve` (camera, look) or a `tag` (figures)."
lanes = "Rows: parts for a figure ({{parts}}); look targets as `{{look_keys_tool}}` names them ({\"graph\": graph, \"node\": id, \"port\": input} or {\"figure\": name, \"slot\": index, \"parameter\": name}). All rows when omitted."
from = "First frame included; the start when omitted."
to = "Last frame included; the end when omitted."
by = "Frames to shift; negative is earlier."
curve = "For `curve`: {{curves}}, or [x1, y1, x2, y2] in 0–127."
tag = "For `tag`: {{tags}}."
+++

Lists, deletes, shifts or re-times one subject's keys within a frame range and set of rows. `list` gives each key's row, frame and tag or curve in frame order; the rest say how many keys changed.

- Shifting onto a frame that holds a key of the same row replaces it.
