{
  "log": {
    "loglevel": "warning"
  },
  "dns": {
    "servers": [
      {
        "address": "https://1.1.1.1/dns-query",
        "port": 443,
        "skipFallback": false,
        "domains": ["domain:samsung.com"]
      },
      {
        "address": "https://dns.google/dns-query",
        "port": 443,
        "skipFallback": false,
        "domains": ["domain:samsung.com"]
      },
      "localhost"
    ],
    "queryStrategy": "UseIP"
  },
  "inbounds": [
    {
      "tag": "tunnel-edge",
      "port": 3080,
      "listen": "0.0.0.0",
      "protocol": "dokodemo-door",
      "settings": {
        "address": "127.0.0.1",
        "port": 8907,
        "network": "tcp",
        "followRedirect": false
      },
      "sniffing": {
        "enabled": false
      }
    }
  ],
  "outbounds": [
    {
      "tag": "vless-tunnel",
      "protocol": "vless",
      "settings": {
        "vnext": [
          {
            "address": "{{BACKEND_HOST}}",
            "port": {{BACKEND_PORT}},
            "users": [
              {
                "id": "{{REALITY_UUID}}",
                "encryption": "none",
                "flow": ""
              }
            ]
          }
        ]
      },
      "streamSettings": {
        "network": "xhttp",
        "xhttpSettings": {
          "path": "/xh"
        },
        "security": "reality",
        "realitySettings": {
          "serverName": "{{REALITY_SERVER_NAME}}",
          "publicKey": "{{REALITY_PUBLIC_KEY}}",
          "shortId": "{{REALITY_SHORT_ID}}",
          "fingerprint": "chrome"
        }
      }
    }
  ],
  "routing": {
    "rules": [
      {
        "type": "field",
        "inboundTag": ["tunnel-edge"],
        "outboundTag": "vless-tunnel"
      }
    ]
  }
}
