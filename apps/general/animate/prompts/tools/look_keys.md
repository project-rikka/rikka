+++
name = "LookKeys"

[params]
set = "Keys to lay; a key on the same frame is replaced."
target = "{\"graph\": graph, \"node\": id, \"port\": input} for a socket, or {\"figure\": name, \"slot\": index or name, \"parameter\": name} for a slot parameter."
frame = "The frame of the key; without one in `remove`, every key of the target."
value = "The keyed value; the current value when omitted."
curve = "Easing from the previous key: {{curves}}, or [x1, y1, x2, y2] in 0–127. Linear when omitted."
remove = "Keys to delete."
+++

Keys look sockets and slot parameters over time as one undo step. Removals come before new keys.
