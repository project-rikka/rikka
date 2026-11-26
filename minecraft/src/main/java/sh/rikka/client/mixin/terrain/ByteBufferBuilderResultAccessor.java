package sh.rikka.client.mixin.terrain;

import com.mojang.blaze3d.vertex.ByteBufferBuilder;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Mutable;
import org.spongepowered.asm.mixin.gen.Accessor;

@Mixin(ByteBufferBuilder.Result.class)
public interface ByteBufferBuilderResultAccessor {
	@Mutable
	@Accessor("size")
	void rikka$setSize(int size);
}
