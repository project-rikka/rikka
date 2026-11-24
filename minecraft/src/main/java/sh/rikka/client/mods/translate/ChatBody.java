package sh.rikka.client.mods.translate;

import java.util.Optional;
import java.util.UUID;
import net.minecraft.network.chat.Component;

record ChatBody(Component content, Optional<UUID> sender) {
}
