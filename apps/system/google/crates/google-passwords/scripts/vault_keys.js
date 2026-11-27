(() => {
  if (window.__rikkaGoogleVaultKeys) return;
  window.__rikkaGoogleVaultKeys = true;
  const DOMAIN = "chromesync";
  const encode = (buffer) => {
    let text = "";
    for (const byte of new Uint8Array(buffer)) text += String.fromCharCode(byte);
    return btoa(text);
  };
  const post = (gaia, keys) => {
    if (!keys.length) return;
    window.postMessage({ "rikka-message": "rikkaGoogleVaultKeys", body: { gaia: String(gaia), keys } }, location.origin);
  };
  const done = (callback) => {
    if (typeof callback === "function") callback();
  };
  const chrome = (window.chrome ??= {});
  chrome.setSyncEncryptionKeys = (callback, gaia, keys) => {
    post(gaia, Array.from(keys ?? [], encode));
    done(callback);
  };
  chrome.setClientEncryptionKeys = (callback, gaia, domains) => {
    const entries = domains instanceof Map ? domains.get(DOMAIN) : undefined;
    post(gaia, Array.from(entries ?? [], (entry) => encode(entry.key)));
    done(callback);
  };
})();
