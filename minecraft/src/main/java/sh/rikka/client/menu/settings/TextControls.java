package sh.rikka.client.menu.settings;

import java.util.ArrayList;
import java.util.IdentityHashMap;
import java.util.List;
import java.util.Map;
import os.rikka.Align;
import os.rikka.Rikka;
import os.rikka.TextDocument;
import os.rikka.View;
import sh.rikka.client.menu.Labels;
import sh.rikka.client.setting.Setting;
import sh.rikka.client.setting.StringListSetting;
import sh.rikka.client.setting.StringSetting;

final class TextControls implements AutoCloseable {
	private final Map<Setting<?, ?>, TextDocument> documents = new IdentityHashMap<>();

	void text(View view, StringSetting setting) {
		TextDocument document = document(setting, setting.get());
		view.textField(document)
			.maxLength(setting.maxLength())
			.fillWidth()
			.onChange(context -> setting.set(document.getText()));
	}

	void list(View view, StringListSetting setting) {
		TextDocument draft = document(setting, "");
		List<String> entries = setting.get();
		view.column(() -> {
				for (int index = 0; index < entries.size(); index++) {
					int position = index;
					view.row(() -> {
							view.text(entries.get(position)).grow(1.0F).shrink(1.0F);
							view.iconButton("close")
								.tooltip(Labels.of("list.remove"))
								.onPress(context -> {
									List<String> next = new ArrayList<>(setting.get());
									next.remove(position);
									setting.set(next);
								});
						})
						.alignItems(Align.CENTER)
						.fillWidth()
						.key("entry:" + index + ":" + entries.get(index));
				}
				view.row(() -> {
						view.textField(draft)
							.placeholder(Labels.of("list.placeholder"))
							.grow(1.0F)
							.onChange(context -> {
							})
							.onSubmit(context -> add(setting, draft));
						view.iconButton("add")
							.tooltip(Labels.of("list.add"))
							.enabled(!draft.getText().isBlank())
							.onPress(context -> add(setting, draft));
					})
					.gap(Rikka.SPACE_SNUG)
					.alignItems(Align.CENTER)
					.fillWidth();
			})
			.gap(Rikka.SPACE_TIGHT)
			.fillWidth();
	}

	private static void add(StringListSetting setting, TextDocument draft) {
		String entry = draft.getText().strip();
		if (entry.isEmpty()) {
			return;
		}
		setting.add(entry);
		draft.reset("");
	}

	private TextDocument document(Setting<?, ?> setting, String text) {
		return this.documents.computeIfAbsent(setting, ignored -> new TextDocument(text));
	}

	void forget(Setting<?, ?> setting) {
		TextDocument document = this.documents.remove(setting);
		if (document != null) {
			document.close();
		}
	}

	@Override
	public void close() {
		for (TextDocument document : this.documents.values()) {
			document.close();
		}
		this.documents.clear();
	}
}
