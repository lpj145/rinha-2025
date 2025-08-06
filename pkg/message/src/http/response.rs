pub static NOT_FOUND: &[u8] = b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n";
pub static BAD_REQUEST: &[u8] = b"HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\n\r\n";
pub static OK: &[u8] = b"HTTP/1.1 200 OK\r\nConnection: Keep-Alive\r\nKeep-Alive: timeout=30, max=500\r\nContent-Length: 0\r\n\r\n";
pub static SUMMARY: &[u8] = b"HTTP/1.1 200 OK\r\nContent-Length: 98\r\n\r\n{\"default\":{\"totalRequests\": 0,\"totalAmount\": 0},\"fallback\":{\"totalRequests\": 0,\"totalAmount\": 0}}";
