package main

import (
	"io"
	"log"
	"net"
	"net/http"
	"os"
	"strings"

	"github.com/gorilla/websocket"
	"github.com/hashicorp/yamux"
)

var (
	uuid     = getEnv("UUID", "d342d11e-d424-4583-b36e-524ab1f0afa4")
	port     = getEnv("PORT", "8080")
	upgrader = websocket.Upgrader{CheckOrigin: func(r *http.Request) bool { return true }}
)

func getEnv(key, def string) string {
	if v := os.Getenv(key); v != "" {
		return v
	}
	return def
}

// Nginx disguise page
const nginxHTML = `<!DOCTYPE html><html><head><title>Welcome to nginx!</title></head><body><h1>Welcome to nginx!</h1><p>If you see this page, the nginx web server is successfully installed and working.</p></body></html>`

func main() {
	http.HandleFunc("/", handler)
	log.Printf("🚀 Proxy server listening on :%s", port)
	log.Printf("🔑 UUID: %s", uuid)
	log.Fatal(http.ListenAndServe(":"+port, nil))
}

func handler(w http.ResponseWriter, r *http.Request) {
	// Check auth via header or path
	proto := r.Header.Get("Sec-WebSocket-Protocol")
	authorized := proto == uuid || strings.Contains(r.URL.Path, uuid)

	if !authorized || !websocket.IsWebSocketUpgrade(r) {
		w.Header().Set("Server", "nginx/1.18.0")
		w.Header().Set("Content-Type", "text/html")
		w.Write([]byte(nginxHTML))
		return
	}

	// Upgrade to WebSocket
	conn, err := upgrader.Upgrade(w, r, http.Header{"Sec-WebSocket-Protocol": {proto}})
	if err != nil {
		log.Printf("❌ Upgrade error: %v", err)
		return
	}
	defer conn.Close()

	log.Println("✅ Client connected")
	handleYamux(conn)
}

// WebSocket adapter for yamux
type wsConn struct {
	*websocket.Conn
	reader io.Reader
}

func (c *wsConn) Read(p []byte) (int, error) {
	for {
		if c.reader == nil {
			_, r, err := c.NextReader()
			if err != nil {
				return 0, err
			}
			c.reader = r
		}
		n, err := c.reader.Read(p)
		if err == io.EOF {
			c.reader = nil
			continue
		}
		return n, err
	}
}

func (c *wsConn) Write(p []byte) (int, error) {
	err := c.WriteMessage(websocket.BinaryMessage, p)
	return len(p), err
}

func handleYamux(conn *websocket.Conn) {
	ws := &wsConn{Conn: conn}

	// Create yamux server session
	cfg := yamux.DefaultConfig()
	cfg.EnableKeepAlive = true
	cfg.KeepAliveInterval = 30

	session, err := yamux.Server(ws, cfg)
	if err != nil {
		log.Printf("❌ Yamux session error: %v", err)
		return
	}
	defer session.Close()

	// Accept streams
	for {
		stream, err := session.Accept()
		if err != nil {
			if err != io.EOF {
				log.Printf("📴 Session closed: %v", err)
			}
			return
		}
		go handleStream(stream)
	}
}

func handleStream(stream net.Conn) {
	defer stream.Close()

	// First read: target address "host:port\n" (newline delimited)
	buf := make([]byte, 512)
	n, err := stream.Read(buf)
	if err != nil {
		return
	}

	data := buf[:n]
	
	// Find newline delimiter
	newlineIdx := -1
	for i, b := range data {
		if b == '\n' {
			newlineIdx = i
			break
		}
	}

	var target string
	var extraData []byte
	
	if newlineIdx >= 0 {
		target = string(data[:newlineIdx])
		if newlineIdx+1 < len(data) {
			extraData = data[newlineIdx+1:]
		}
	} else {
		// Fallback: no newline, treat entire data as target
		target = strings.TrimSpace(string(data))
	}

	parts := strings.SplitN(target, ":", 2)
	if len(parts) != 2 {
		log.Printf("❌ Invalid target: %s", target)
		return
	}

	host, port := parts[0], parts[1]
	log.Printf("🔗 Connecting to %s:%s", host, port)

	// Connect to target
	remote, err := net.Dial("tcp", host+":"+port)
	if err != nil {
		log.Printf("❌ Dial error: %v", err)
		return
	}
	defer remote.Close()

	log.Printf("✅ Connected to %s:%s", host, port)

	// Send extra data that came with target address (e.g., HTTP request)
	if len(extraData) > 0 {
		remote.Write(extraData)
	}

	// Bidirectional copy
	done := make(chan struct{})
	go func() {
		io.Copy(remote, stream)
		done <- struct{}{}
	}()
	go func() {
		io.Copy(stream, remote)
		done <- struct{}{}
	}()
	<-done
}
