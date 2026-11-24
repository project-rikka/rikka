package sh.rikka.client.mods.translate;

import java.util.List;
import java.util.Optional;
import java.util.concurrent.CompletableFuture;
import net.minecraft.ChatFormatting;
import net.minecraft.client.Minecraft;
import net.minecraft.client.multiplayer.ClientPacketListener;
import net.minecraft.network.chat.ClickEvent;
import net.minecraft.network.chat.Component;
import net.minecraft.network.chat.HoverEvent;
import net.minecraft.network.chat.Style;
import net.minecraft.util.StringUtil;
import org.apache.commons.lang3.StringUtils;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import sh.rikka.client.translate.Languages;
import sh.rikka.client.translate.Translated;
import sh.rikka.client.translate.TranslationFailure;
import sh.rikka.client.translate.TranslationFailure.Reason;
import sh.rikka.client.translate.TranslationQueue;
import sh.rikka.client.translate.Urgency;
import sh.rikka.client.translate.markup.Markup;
import sh.rikka.client.translate.markup.Segment;

final class OutgoingTranslations {
	private static final Logger LOGGER = LoggerFactory.getLogger(OutgoingTranslations.class);
	private static final int MAX_MESSAGE_LENGTH = 256;

	private final ChatTranslateMod mod;
	private CompletableFuture<Void> tail = CompletableFuture.completedFuture(null);

	OutgoingTranslations(ChatTranslateMod mod) {
		this.mod = mod;
	}

	boolean send(String message, boolean addToRecent) {
		if (!this.mod.translatesOutgoing() || message.isEmpty()) {
			return false;
		}
		Minecraft minecraft = Minecraft.getInstance();
		ClientPacketListener connection = minecraft.getConnection();
		if (connection == null) {
			return false;
		}
		Optional<OutgoingText> text = OutgoingText.parse(message)
			.filter(parsed -> ComponentMarkup.hasLetters(List.of(segment(parsed.body()))));
		if (text.isEmpty() && this.tail.isDone()) {
			return false;
		}
		if (addToRecent) {
			minecraft.gui.hud.getChat().addRecentChat(message);
		}
		CompletableFuture<Outcome> outcome = text
			.map(this::translate)
			.orElseGet(() -> CompletableFuture.completedFuture(new Outcome.Sent(message)));
		this.tail = this.tail
			.thenCombine(outcome, (previous, result) -> result)
			.thenAccept(result -> deliver(connection, message, result))
			.exceptionally(error -> {
				LOGGER.error("failed to send translated message", error);
				return null;
			});
		return true;
	}

	private CompletableFuture<Outcome> translate(OutgoingText text) {
		String target = this.mod.outgoingTarget();
		Markup<Style> markup = Markup.encode(List.of(segment(text.body())), this.mod.protection());
		return this.mod.queue().request(markup.html(), target, Urgency.USER)
			.handle((translated, error) -> error == null
				? outcome(text, markup, translated, target)
				: new Outcome.Failed(TranslationQueue.failure(error)));
	}

	private static Segment<Style> segment(String text) {
		return new Segment<>(text, Style.EMPTY, false);
	}

	private static Outcome outcome(OutgoingText text, Markup<Style> markup, Translated translated, String target) {
		if (translated.detected().filter(detected -> Languages.sameLanguage(detected, target)).isPresent()) {
			return new Outcome.Sent(text.with(text.body()));
		}
		String body = StringUtil.filterText(StringUtils.normalizeSpace(Segment.text(markup.decode(translated.html()))));
		if (body.isBlank()) {
			return new Outcome.Failed(new TranslationFailure(Reason.MALFORMED, "translation is empty"));
		}
		String full = text.with(body);
		if (full.length() > MAX_MESSAGE_LENGTH) {
			return new Outcome.Failed(new TranslationFailure(Reason.TOO_LONG, "translation exceeds " + MAX_MESSAGE_LENGTH + " characters"));
		}
		return new Outcome.Sent(full);
	}

	private static void deliver(ClientPacketListener connection, String original, Outcome outcome) {
		Minecraft minecraft = Minecraft.getInstance();
		if (minecraft.getConnection() != connection) {
			LOGGER.info("dropped a translated message: the connection closed");
			return;
		}
		switch (outcome) {
			case Outcome.Sent sent -> {
				if (sent.message().startsWith(OutgoingText.COMMAND_PREFIX)) {
					connection.sendCommand(sent.message().substring(OutgoingText.COMMAND_PREFIX.length()));
				} else {
					connection.sendChat(sent.message());
				}
			}
			case Outcome.Failed failed -> minecraft.gui.hud.getChat().addClientSystemMessage(failure(original, failed.failure()));
		}
	}

	private static Component failure(String original, TranslationFailure failure) {
		Component edit = Texts.of("send.edit").setStyle(Style.EMPTY
			.withColor(ChatFormatting.AQUA)
			.withClickEvent(new ClickEvent.SuggestCommand(original))
			.withHoverEvent(new HoverEvent.ShowText(Component.literal(original))));
		return Component.empty()
			.append(Texts.of("send.failed", Texts.failure(failure)).withStyle(ChatFormatting.RED))
			.append(" ")
			.append(Component.literal("[").append(edit).append("]").withStyle(ChatFormatting.AQUA));
	}

	private sealed interface Outcome {
		record Sent(String message) implements Outcome {
		}

		record Failed(TranslationFailure failure) implements Outcome {
		}
	}
}
