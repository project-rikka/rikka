package sh.rikka.client.events;

import java.util.OptionalInt;
import net.minecraft.client.player.LocalPlayer;
import sh.rikka.client.event.Event;

public record HurtEvent(LocalPlayer player, OptionalInt attackerId) implements Event {
}
