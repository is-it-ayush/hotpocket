### hotpocket.

A raw rust web server that works. You can fork and expand
on this project to your server needs.

### performance.

- `wrk -t8 -c500 -d30s "http://127.0.0.1:3000/"```:
  - 15654.47 requests/sec or about 1,352,546,208
  requests/day (yeah, a billion/day is a lot haha!)
  - 63.25MB read
  - 2.11MB transfer/sec
```sh
Running 30s test @ http://127.0.0.1:3000/
  8 threads and 500 connections
  Thread Stats   Avg      Stdev     Max   +/- Stdev
    Latency    19.97ms   97.66ms   1.69s    97.62%
    Req/Sec     2.01k   744.62     5.87k    73.15%
  470406 requests in 30.05s, 63.25MB read
  Socket errors: connect 0, read 4, write 0, timeout 157
Requests/sec:  15654.47
Transfer/sec:      2.11MB
```

### usage.

It's literally a raw server that parses the HTTP request and
sends back a HTTP response. You can fork and change as you see
fit.

### license.

MIT License. You can read it [here](./LICENSE.md)!
