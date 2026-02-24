/*! coi-serviceworker v0.1.7 - Guido Zuidhof and contributors, licensed under MIT */
let coepCredentialless = false;
if (typeof window === 'undefined') {
    self.addEventListener("install", () => self.skipWaiting());
    self.addEventListener("activate", (e) => e.waitUntil(self.clients.claim()));
    self.addEventListener("message", (ev) => {
        if (ev.data && ev.data.type === "deregister") {
            self.registration
                .unregister()
                .then(() => {
                    return self.clients.matchAll();
                })
                .then((clients) => {
                    clients.forEach((client) => client.navigate(client.url));
                });
        }
    });
    self.addEventListener("fetch", function (event) {
        const r = event.request;
        if (r.cache === "only-if-cached" && r.mode !== "same-origin") {
            return;
        }
        const request =
            coepCredentialless && r.mode === "no-cors"
                ? new Request(r, {
                      credentials: "omit",
                  })
                : r;
        event.respondWith(
            fetch(request)
                .then((response) => {
                    if (response.status === 0) {
                        return response;
                    }
                    const newHeaders = new Headers(response.headers);
                    newHeaders.set(
                        "Cross-Origin-Embedder-Policy",
                        coepCredentialless ? "credentialless" : "require-corp"
                    );
                    newHeaders.set("Cross-Origin-Opener-Policy", "same-origin");
                    return new Response(response.body, {
                        status: response.status,
                        statusText: response.statusText,
                        headers: newHeaders,
                    });
                })
                .catch((e) => console.error(e))
        );
    });
} else {
    (() => {
        const reloadedBySelf = window.sessionStorage.getItem("coiReloadedBySelf");
        window.sessionStorage.removeItem("coiReloadedBySelf");
        const coepDegrading = reloadedBySelf === "coepdegrade";
        if (window.crossOriginIsolated !== false || reloadedBySelf) {
            return;
        }
        if (!window.isSecureContext) {
            !coepDegrading &&
                console.log(
                    "COOP/COEP Service Worker: Not installing, not in a secure context."
                );
            return;
        }
        if (navigator.serviceWorker) {
            navigator.serviceWorker
                .register(window.document.currentScript.src)
                .then(
                    (registration) => {
                        !coepDegrading &&
                            console.log(
                                "COOP/COEP Service Worker: Registered.",
                                registration.scope
                            );
                        registration.addEventListener("updatefound", () => {
                            !coepDegrading &&
                                console.log(
                                    "COOP/COEP Service Worker: New version installing..."
                                );
                            registration.installing.addEventListener(
                                "statechange",
                                function () {
                                    if (this.state === "installed") {
                                        !coepDegrading &&
                                            console.log(
                                                "COOP/COEP Service Worker: Installed. Reloading page."
                                            );
                                        window.sessionStorage.setItem(
                                            "coiReloadedBySelf",
                                            "true"
                                        );
                                        window.location.reload();
                                    }
                                }
                            );
                        });
                    },
                    (err) => {
                        !coepDegrading &&
                            console.error(
                                "COOP/COEP Service Worker: Registration failed.",
                                err
                            );
                    }
                );
        }
    })();
}
