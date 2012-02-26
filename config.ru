# This file is used by Rack-based servers to start the application.

require ::File.expand_path('../config/environment',  __FILE__)
Golaberto::Application.config.middleware.use ExceptionNotifier,
  :email_prefix => "[GolAberto] ",
  :sender_address => %{"notifier" <notifier@golaberto.com.br>},
  :exception_recipients => %w{golaberto@gmail.com}

run Golaberto::Application
