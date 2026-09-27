package main

import (
	"fmt"
	"io"
	"net/url"
	"os"
	"strings"

	http "github.com/bogdanfinn/fhttp"
	tlsclient "github.com/bogdanfinn/tls-client"
	"github.com/bogdanfinn/tls-client/profiles"
)

func main() {
	if len(os.Args) != 2 {
		fail("usage: sofascore_fetch https://www.sofascore.com/api/v1/...")
	}

	target, err := url.Parse(os.Args[1])
	if err != nil || target.Scheme != "https" ||
		(target.Hostname() != "www.sofascore.com" && target.Hostname() != "sofascore.com" && target.Hostname() != "api.sofascore.com") ||
		!strings.HasPrefix(target.Path, "/api/v1/") {
		fail("expected a SofaScore HTTPS API URL")
	}

	client, err := tlsclient.NewHttpClient(tlsclient.NewNoopLogger(),
		tlsclient.WithTimeoutSeconds(15),
		tlsclient.WithClientProfile(profiles.Chrome_152),
		tlsclient.WithDisableHttp3())
	if err != nil {
		fail("create HTTP client: %v", err)
	}

	request, err := http.NewRequest(http.MethodGet, target.String(), nil)
	if err != nil {
		fail("create request: %v", err)
	}
	request.Header = http.Header{
		"accept":           {"application/json"},
		"accept-language":  {"en-US,en;q=0.9"},
		"referer":          {"https://www.sofascore.com/"},
		"user-agent":       {"Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/152.0.0.0 Safari/537.36"},
		"x-requested-with": {"XMLHttpRequest"},
	}

	response, err := client.Do(request)
	if err != nil {
		fail("request failed: %v", err)
	}
	defer response.Body.Close()

	if response.StatusCode < 200 || response.StatusCode >= 300 {
		body, _ := io.ReadAll(io.LimitReader(response.Body, 200))
		fail("HTTP %d: %s", response.StatusCode, strings.TrimSpace(string(body)))
	}

	if _, err := io.Copy(os.Stdout, response.Body); err != nil {
		fail("read response: %v", err)
	}
}

func fail(format string, args ...any) {
	fmt.Fprintf(os.Stderr, format+"\n", args...)
	os.Exit(1)
}
