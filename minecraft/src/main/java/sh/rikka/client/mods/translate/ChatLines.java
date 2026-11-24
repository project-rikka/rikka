package sh.rikka.client.mods.translate;

import java.util.List;
import java.util.Map;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.components.ChatComponent;
import net.minecraft.client.multiplayer.chat.GuiMessage;
import sh.rikka.client.mixin.ChatComponentAccessor;

final class ChatLines {
	private ChatLines() {
	}

	static boolean contains(GuiMessage message) {
		return indexOf(messages(chat()), message) >= 0;
	}

	static void replace(Map<GuiMessage, GuiMessage> replacements) {
		ChatComponent chat = chat();
		List<GuiMessage> messages = messages(chat);
		boolean changed = false;
		for (Map.Entry<GuiMessage, GuiMessage> replacement : replacements.entrySet()) {
			int index = indexOf(messages, replacement.getKey());
			if (index >= 0) {
				messages.set(index, replacement.getValue());
				changed = true;
			}
		}
		if (changed) {
			refresh(chat);
		}
	}

	private static void refresh(ChatComponent chat) {
		ChatComponentAccessor accessor = (ChatComponentAccessor) chat;
		int scrolled = accessor.rikka$chatScrollbarPos();
		boolean unread = accessor.rikka$newMessageSinceScroll();
		accessor.rikka$refreshTrimmedMessages();
		accessor.rikka$setChatScrollbarPos(0);
		chat.scrollChat(scrolled);
		accessor.rikka$setNewMessageSinceScroll(unread && accessor.rikka$chatScrollbarPos() > 0);
	}

	private static int indexOf(List<GuiMessage> messages, GuiMessage message) {
		for (int index = 0; index < messages.size(); index++) {
			if (messages.get(index) == message) {
				return index;
			}
		}
		return -1;
	}

	private static List<GuiMessage> messages(ChatComponent chat) {
		return ((ChatComponentAccessor) chat).rikka$allMessages();
	}

	private static ChatComponent chat() {
		return Minecraft.getInstance().gui.hud.getChat();
	}
}
