package sh.rikka.client.mods.privacy;

import java.util.Locale;
import java.util.UUID;
import net.minecraft.client.resources.DefaultPlayerSkin;
import net.minecraft.core.ClientAsset;
import net.minecraft.resources.Identifier;
import net.minecraft.world.entity.player.PlayerModelType;
import net.minecraft.world.entity.player.PlayerSkin;
import org.jspecify.annotations.Nullable;

public enum SkinChoice {
	KEEP,
	DEFAULT,
	STEVE,
	ALEX,
	ARI,
	EFE,
	KAI,
	MAKENA,
	NOOR,
	SUNNY,
	ZURI;

	@Nullable PlayerSkin skin(UUID profile, boolean slim) {
		return switch (this) {
			case KEEP -> null;
			case DEFAULT -> DefaultPlayerSkin.get(profile);
			default -> {
				PlayerModelType model = slim ? PlayerModelType.SLIM : PlayerModelType.WIDE;
				String texture = "entity/player/" + model.getSerializedName() + "/" + name().toLowerCase(Locale.ROOT);
				yield new PlayerSkin(new ClientAsset.ResourceTexture(Identifier.withDefaultNamespace(texture)), null, null, model, true);
			}
		};
	}

	boolean isNamed() {
		return this != KEEP && this != DEFAULT;
	}
}
