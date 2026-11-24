package sh.rikka.client.mods.translate;

import java.util.List;
import java.util.Optional;
import net.minecraft.client.multiplayer.chat.GuiMessage;
import net.minecraft.network.chat.Component;
import net.minecraft.network.chat.Style;
import org.jspecify.annotations.Nullable;
import sh.rikka.client.translate.TranslationFailure;
import sh.rikka.client.translate.markup.Segment;

final class MessageEntry {
	private final int id;
	private final Component original;
	private final List<Segment<Style>> body;
	private GuiMessage message;
	private State state = State.READY;
	private boolean requested;
	private @Nullable Component translation;
	private @Nullable String source;
	private @Nullable String target;
	private @Nullable TranslationFailure failure;

	MessageEntry(int id, GuiMessage message, List<Segment<Style>> body) {
		this.id = id;
		this.original = message.content();
		this.message = message;
		this.body = body;
	}

	int id() {
		return this.id;
	}

	Component original() {
		return this.original;
	}

	List<Segment<Style>> body() {
		return this.body;
	}

	GuiMessage message() {
		return this.message;
	}

	void attach(GuiMessage message) {
		this.message = message;
	}

	GuiMessage with(Component content) {
		return new GuiMessage(this.message.addedTime(), content, this.message.signature(), this.message.source(), this.message.tag());
	}

	State state() {
		return this.state;
	}

	boolean requested() {
		return this.requested;
	}

	void request() {
		this.requested = true;
	}

	void dismiss() {
		this.requested = false;
	}

	void translating() {
		this.state = State.TRANSLATING;
		this.failure = null;
	}

	void translated(Component translation, Optional<String> source) {
		this.state = State.SHOWN;
		this.translation = translation;
		this.source = source.orElse(null);
	}

	void same(String target) {
		this.state = State.SAME;
		this.target = target;
	}

	void failed(TranslationFailure failure) {
		this.state = State.FAILED;
		this.failure = failure;
	}

	void show() {
		if (this.state == State.HIDDEN) {
			this.state = State.SHOWN;
		}
	}

	void hide() {
		if (this.state == State.SHOWN) {
			this.state = State.HIDDEN;
		}
	}

	Optional<Component> translation() {
		return Optional.ofNullable(this.translation);
	}

	Optional<String> source() {
		return Optional.ofNullable(this.source);
	}

	Optional<String> target() {
		return Optional.ofNullable(this.target);
	}

	Optional<TranslationFailure> failure() {
		return Optional.ofNullable(this.failure);
	}

	enum State {
		READY,
		TRANSLATING,
		SHOWN,
		HIDDEN,
		SAME,
		FAILED
	}
}
