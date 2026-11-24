package sh.rikka.client.mods.translate;

import java.util.List;
import java.util.concurrent.Executors;
import net.minecraft.client.Minecraft;
import org.jspecify.annotations.Nullable;
import sh.rikka.client.event.EventHandler;
import sh.rikka.client.events.TickEvent;
import sh.rikka.client.mod.Category;
import sh.rikka.client.mod.Mod;
import sh.rikka.client.setting.BoolSetting;
import sh.rikka.client.setting.LanguageListSetting;
import sh.rikka.client.setting.LanguageSetting;
import sh.rikka.client.translate.Languages;
import sh.rikka.client.translate.TranslationQueue;
import sh.rikka.client.translate.markup.Protection;

public final class ChatTranslateMod extends Mod {
	static final String ID = "chat_translate";
	private static final String WORKER_NAME = "rikka-translate-";
	private static volatile @Nullable ChatTranslateMod active;

	private final LanguageSetting target = languageOrGame("target");
	private final BoolSetting automatic = bool("auto_translate", false);
	private final BoolSetting button = bool("button", true);
	private final BoolSetting serverMessages = bool("server_messages", true);
	private final BoolSetting outgoing = bool("outgoing", false);
	private final LanguageSetting outgoingLanguage = language("outgoing_language", Languages.FALLBACK);
	private final LanguageListSetting outgoingLanguages = languages("outgoing_languages", Languages.FALLBACK);

	private final IncomingTranslations incoming = new IncomingTranslations(this);
	private final OutgoingTranslations sending = new OutgoingTranslations(this);
	private final PlayerNames playerNames = new PlayerNames();
	private @Nullable TranslationQueue queue;

	public ChatTranslateMod() {
		super(ID, Category.UTILITY, "translate");
		this.button.onChange(shown -> this.incoming.rerenderAll());
	}

	static @Nullable ChatTranslateMod active() {
		return active;
	}

	@Override
	protected void onEnable() {
		active = this;
	}

	@Override
	protected void onDisable() {
		active = null;
		this.incoming.clear();
	}

	@EventHandler
	private void onTick(TickEvent.Post event) {
		this.playerNames.invalidate();
		TranslationQueue current = this.queue;
		if (current != null) {
			current.flush();
		}
		this.incoming.publish();
	}

	IncomingTranslations incoming() {
		return this.incoming;
	}

	OutgoingTranslations sending() {
		return this.sending;
	}

	Protection protection() {
		return this.playerNames.protection();
	}

	TranslationQueue queue() {
		TranslationQueue current = this.queue;
		if (current == null) {
			current = new TranslationQueue(Executors.newThreadPerTaskExecutor(Thread.ofVirtual().name(WORKER_NAME, 0).factory()), Minecraft.getInstance());
			this.queue = current;
		}
		return current;
	}

	String target() {
		return this.target.resolved();
	}

	boolean translatesAutomatically() {
		return this.automatic.get();
	}

	boolean showsButton() {
		return this.button.get();
	}

	boolean translatesServerMessages() {
		return this.serverMessages.get();
	}

	boolean translatesOutgoing() {
		return this.outgoing.get();
	}

	void toggleOutgoing() {
		this.outgoing.set(!this.outgoing.get());
	}

	String outgoingTarget() {
		return this.outgoingLanguage.get();
	}

	void cycleOutgoingLanguage(int step) {
		List<String> languages = this.outgoingLanguages.get();
		if (languages.isEmpty()) {
			return;
		}
		int index = languages.indexOf(this.outgoingLanguage.get());
		int next = index < 0 ? (step > 0 ? 0 : languages.size() - 1) : Math.floorMod(index + step, languages.size());
		this.outgoingLanguage.set(languages.get(next));
	}
}
