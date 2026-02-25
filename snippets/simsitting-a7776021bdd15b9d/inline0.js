
        export function get_psychographic_data() {
            return {
                hour: new Date().getHours(),
                timezone: Intl.DateTimeFormat().resolvedOptions().timeZone,
                language: navigator.language,
                platform: navigator.platform || 'Unknown',
                is_hdr: window.matchMedia('(dynamic-range: high)').matches,
            };
        }
    