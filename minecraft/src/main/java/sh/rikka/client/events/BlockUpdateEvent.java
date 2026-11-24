package sh.rikka.client.events;

import net.minecraft.core.BlockPos;
import sh.rikka.client.event.Event;

public record BlockUpdateEvent(BlockPos pos) implements Event {
}
