+++
name = "Save"

[params]
document = "Tab name; the active tab when omitted."
path = "Absolute path in the home folder, its extension choosing the format: {{formats}}. The tab's own file when omitted."
quality = "1–100 for JPEG and AVIF; the window's last choice when omitted."
+++

Writes the document. PSD keeps layers; flat formats write the visible picture, and a layered document saved flat stays bound to its own file. Save only when asked, to a new path unless told to overwrite.
