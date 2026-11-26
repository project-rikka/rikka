package sh.rikka.client.startup;

import java.security.KeyPair;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.CompletionException;
import net.minecraft.util.Crypt;
import net.minecraft.util.CryptException;

public final class ServerKeyPairs {
	private static final String THREAD_NAME = "Rikka Key Pair Generator";

	private static CompletableFuture<KeyPair> pending;

	private ServerKeyPairs() {
	}

	public static synchronized void prepare() {
		pending = generate();
	}

	public static KeyPair take() throws CryptException {
		CompletableFuture<KeyPair> current;
		synchronized (ServerKeyPairs.class) {
			current = pending;
			pending = generate();
		}
		try {
			return current.join();
		} catch (CompletionException exception) {
			switch (exception.getCause()) {
				case CryptException crypt -> throw crypt;
				case RuntimeException runtime -> throw runtime;
				case Error error -> throw error;
				default -> throw exception;
			}
		}
	}

	private static CompletableFuture<KeyPair> generate() {
		CompletableFuture<KeyPair> keyPair = new CompletableFuture<>();
		Thread.ofPlatform().daemon().name(THREAD_NAME).start(() -> {
			try {
				keyPair.complete(Crypt.generateKeyPair());
			} catch (Throwable throwable) {
				keyPair.completeExceptionally(throwable);
			}
		});
		return keyPair;
	}
}
