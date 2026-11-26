package sh.rikka.client.mixin.culling;

import net.minecraft.world.entity.Entity;
import org.jspecify.annotations.Nullable;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Unique;
import sh.rikka.client.render.culling.CullTicket;
import sh.rikka.client.render.culling.Occludable;

@Mixin(Entity.class)
abstract class EntityMixin implements Occludable {
	@Unique
	private @Nullable CullTicket rikka$ticket;

	@Override
	public CullTicket rikka$cullTicket() {
		CullTicket ticket = this.rikka$ticket;
		if (ticket == null) {
			ticket = new CullTicket();
			this.rikka$ticket = ticket;
		}
		return ticket;
	}
}
