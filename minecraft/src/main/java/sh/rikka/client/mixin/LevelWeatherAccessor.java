package sh.rikka.client.mixin;

import net.minecraft.world.level.Level;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.gen.Accessor;

@Mixin(Level.class)
public interface LevelWeatherAccessor {
	@Accessor("rainLevel")
	float rikka$rainLevel();

	@Accessor("thunderLevel")
	float rikka$thunderLevel();
}
