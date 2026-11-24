package sh.rikka.client.mods.translate;

import java.util.IdentityHashMap;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.Set;
import java.util.UUID;
import java.util.WeakHashMap;
import net.minecraft.ChatFormatting;
import net.minecraft.client.Minecraft;
import net.minecraft.client.multiplayer.chat.GuiMessage;
import net.minecraft.client.multiplayer.chat.GuiMessageSource;
import net.minecraft.client.multiplayer.chat.GuiMessageTag;
import net.minecraft.nbt.IntTag;
import net.minecraft.nbt.Tag;
import net.minecraft.network.chat.ClickEvent;
import net.minecraft.network.chat.Component;
import net.minecraft.network.chat.HoverEvent;
import net.minecraft.network.chat.MutableComponent;
import net.minecraft.network.chat.Style;
import net.minecraft.resources.Identifier;
import sh.rikka.client.RikkaClient;
import sh.rikka.client.translate.Languages;
import sh.rikka.client.translate.Translated;
import sh.rikka.client.translate.TranslationFailure;
import sh.rikka.client.translate.TranslationQueue;
import sh.rikka.client.translate.Urgency;
import sh.rikka.client.translate.markup.Markup;
import sh.rikka.client.translate.markup.Segment;

final class IncomingTranslations {
	static final Identifier BUTTON = Identifier.fromNamespaceAndPath(RikkaClient.MOD_ID, "translate");

	private static final int MAX_ENTRIES = 256;
	private static final String BUTTON_GLYPH = "[✎]";
	private static final String BUSY_GLYPH = "[…]";
	private static final String LINE_PREFIX = " ↳ ";

	private final ChatTranslateMod mod;
	private final Map<Component, ChatBody> bodies = new WeakHashMap<>();
	private final Map<Integer, MessageEntry> entries = new LinkedHashMap<>() {
		private static final long serialVersionUID = 1L;

		@Override
		protected boolean removeEldestEntry(Map.Entry<Integer, MessageEntry> eldest) {
			return size() > MAX_ENTRIES;
		}
	};
	private final Set<MessageEntry> changed = new LinkedHashSet<>();
	private int nextId;

	IncomingTranslations(ChatTranslateMod mod) {
		this.mod = mod;
	}

	void body(Component decorated, Component content, Optional<UUID> sender) {
		this.bodies.put(decorated, new ChatBody(content, sender));
	}

	GuiMessage decorate(GuiMessage message) {
		ChatBody body = this.bodies.remove(message.content());
		if (!eligible(message) || !(this.mod.showsButton() || this.mod.translatesAutomatically())) {
			return message;
		}
		List<Segment<Style>> segments = ComponentMarkup.segments(body != null ? body.content() : message.content());
		if (!ComponentMarkup.hasLetters(segments)) {
			return message;
		}
		MessageEntry entry = new MessageEntry(this.nextId++, message, segments);
		this.entries.put(entry.id(), entry);
		boolean own = body != null && body.sender().filter(IncomingTranslations::isLocalPlayer).isPresent();
		if (this.mod.translatesAutomatically() && !own) {
			translate(entry, Urgency.BACKGROUND);
		}
		this.changed.remove(entry);
		GuiMessage decorated = entry.with(render(entry));
		entry.attach(decorated);
		return decorated;
	}

	void click(Optional<Tag> payload) {
		if (!(payload.orElse(null) instanceof IntTag tag)) {
			return;
		}
		MessageEntry entry = this.entries.get(tag.intValue());
		if (entry == null || !ChatLines.contains(entry.message())) {
			return;
		}
		switch (entry.state()) {
			case READY, FAILED -> {
				entry.request();
				translate(entry, Urgency.USER);
			}
			case TRANSLATING -> entry.request();
			case SHOWN -> entry.hide();
			case HIDDEN -> entry.show();
			case SAME -> {
				if (entry.requested()) {
					entry.dismiss();
				} else {
					entry.request();
				}
			}
		}
		rerender(List.of(entry));
	}

	void rerenderAll() {
		rerender(this.entries.values());
	}

	void publish() {
		if (!this.changed.isEmpty()) {
			rerender(List.copyOf(this.changed));
		}
	}

	void clear() {
		Map<GuiMessage, GuiMessage> replacements = new IdentityHashMap<>();
		for (MessageEntry entry : this.entries.values()) {
			replacements.put(entry.message(), entry.with(entry.original()));
		}
		this.entries.clear();
		this.changed.clear();
		this.bodies.clear();
		if (!replacements.isEmpty()) {
			ChatLines.replace(replacements);
		}
	}

	private boolean eligible(GuiMessage message) {
		if (message.tag() == GuiMessageTag.chatError()) {
			return false;
		}
		return message.source() == GuiMessageSource.PLAYER || (message.source() == GuiMessageSource.SYSTEM_SERVER && this.mod.translatesServerMessages());
	}

	private static boolean isLocalPlayer(UUID sender) {
		return sender.equals(Minecraft.getInstance().getUser().getProfileId());
	}

	private void translate(MessageEntry entry, Urgency urgency) {
		String target = this.mod.target();
		Markup<Style> markup = Markup.encode(entry.body(), this.mod.protection());
		TranslationQueue queue = this.mod.queue();
		entry.translating();
		queue.request(markup.html(), target, urgency).whenComplete((translated, error) -> {
			if (error != null) {
				entry.failed(TranslationQueue.failure(error));
			} else {
				apply(entry, markup, translated, target);
			}
			if (this.entries.get(entry.id()) == entry) {
				this.changed.add(entry);
			}
		});
	}

	private static void apply(MessageEntry entry, Markup<Style> markup, Translated translated, String target) {
		if (translated.detected().filter(detected -> Languages.sameLanguage(detected, target)).isPresent()) {
			entry.same(target);
			return;
		}
		List<Segment<Style>> segments = markup.decode(translated.html());
		String text = Segment.text(segments);
		if (text.isBlank() || text.strip().equalsIgnoreCase(Segment.text(entry.body()).strip())) {
			entry.same(target);
			return;
		}
		entry.translated(ComponentMarkup.component(segments), translated.detected());
	}

	private void rerender(Iterable<MessageEntry> entries) {
		Map<GuiMessage, GuiMessage> replacements = new IdentityHashMap<>();
		Map<MessageEntry, GuiMessage> rendered = new IdentityHashMap<>();
		for (MessageEntry entry : entries) {
			GuiMessage replacement = entry.with(render(entry));
			replacements.put(entry.message(), replacement);
			rendered.put(entry, replacement);
		}
		this.changed.removeAll(rendered.keySet());
		if (replacements.isEmpty()) {
			return;
		}
		ChatLines.replace(replacements);
		rendered.forEach(MessageEntry::attach);
	}

	private Component render(MessageEntry entry) {
		MutableComponent content = Component.empty().append(entry.original());
		if (this.mod.showsButton()) {
			content.append(" ").append(button(entry));
		}
		switch (entry.state()) {
			case SHOWN -> entry.translation().ifPresent(translation -> line(content, translatedPrefix(entry)).append(translation));
			case SAME -> {
				if (entry.requested()) {
					line(content, notePrefix()).append(Texts.of("already", Texts.languageName(entry.target().orElseThrow())).withStyle(ChatFormatting.GRAY, ChatFormatting.ITALIC));
				}
			}
			case FAILED -> {
				if (entry.requested()) {
					line(content, notePrefix()).append(Texts.failure(entry.failure().orElseThrow()).withStyle(ChatFormatting.RED));
				}
			}
			case READY, TRANSLATING, HIDDEN -> {
			}
		}
		return content;
	}

	private static MutableComponent line(MutableComponent content, Component prefix) {
		return content.append("\n").append(prefix);
	}

	private static Component translatedPrefix(MessageEntry entry) {
		Component hover = entry.source()
			.map(source -> Texts.of("translated_from", Texts.languageName(source)))
			.orElseGet(() -> Texts.of("translated"));
		return Component.literal(LINE_PREFIX).withStyle(Style.EMPTY.withColor(ChatFormatting.DARK_GRAY).withHoverEvent(new HoverEvent.ShowText(hover)));
	}

	private static Component notePrefix() {
		return Component.literal(LINE_PREFIX).withStyle(ChatFormatting.DARK_GRAY);
	}

	private static Component button(MessageEntry entry) {
		ChatFormatting color;
		Component hover;
		String glyph = BUTTON_GLYPH;
		switch (entry.state()) {
			case READY -> {
				color = ChatFormatting.GRAY;
				hover = Texts.of("button.translate");
			}
			case TRANSLATING -> {
				color = ChatFormatting.DARK_GRAY;
				hover = Texts.of("button.translating");
				glyph = BUSY_GLYPH;
			}
			case SHOWN -> {
				color = ChatFormatting.GREEN;
				hover = Texts.of("button.hide");
			}
			case HIDDEN -> {
				color = ChatFormatting.GRAY;
				hover = Texts.of("button.show");
			}
			case SAME -> {
				color = ChatFormatting.GRAY;
				hover = Texts.of("already", Texts.languageName(entry.target().orElseThrow()));
			}
			case FAILED -> {
				color = ChatFormatting.RED;
				hover = Texts.of("button.retry", Texts.failure(entry.failure().orElseThrow()));
			}
			default -> throw new IllegalStateException("unknown state " + entry.state());
		}
		Style style = Style.EMPTY
			.withColor(color)
			.withClickEvent(new ClickEvent.Custom(BUTTON, Optional.of(IntTag.valueOf(entry.id()))))
			.withHoverEvent(new HoverEvent.ShowText(hover));
		return Component.literal(glyph).setStyle(style);
	}
}
