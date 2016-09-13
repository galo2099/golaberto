class ConvertDatetimeToUTC < ActiveRecord::Migration
  TZ_CHAMP = { ActiveSupport::TimeZone.new("Europe/London") => Championship.where("name LIKE 'England%' OR name = 'Champions League' OR name = 'Europa League' OR name = 'UEFA Cup' OR name = 'Dutch Eredivisie' OR name = 'France Ligue 1' OR name = 'Scotland Premier'").map(&:id),
               ActiveSupport::TimeZone.new("Europe/Madrid") => Championship.where("name LIKE 'España%'").map(&:id),
               ActiveSupport::TimeZone.new("Europe/Berlin") => Championship.where("name LIKE 'Bundesliga%'").map(&:id),
               ActiveSupport::TimeZone.new("Europe/Lisbon") => Championship.where("name LIKE 'Portugal%'").map(&:id),
               ActiveSupport::TimeZone.new("Europe/Stockholm") => Championship.where("name LIKE '%Swedish%'").map(&:id),
               ActiveSupport::TimeZone.new("Asia/Muscat") => Championship.where("name = 'FIFA Club World Cup UAE'").map(&:id),
               ActiveSupport::TimeZone.new("America/New_York") => Championship.where("name = 'MLS'").map(&:id),
               ActiveSupport::TimeZone.new("America/Sao_Paulo") => Championship.where("name LIKE 'Campeonato%' OR name LIKE 'Copa%' OR name LIKE '%Torneio%' OR name LIKE '%Taça%' OR name LIKE '%Minei%' OR name LIKE '%Troféu%' OR name LIKE '%Série%' OR name LIKE '%Minas%' OR name = 'Recopa Sulamericana' OR name = 'Liga Futsal' OR name = 'Future Champions' OR name = 'AFC Asian Cup' OR name = 'Eliminatória Sulamericana Mundial FIFA Brasil 2014' OR name = 'Mundial de Clubes' OR name = 'World Cup 2010 South America Qualifying'").map(&:id) }
  def up
    [[:games, Game]].each do |g|
      TZ_CHAMP.each do |tz, champs|
        say_with_time "Updating games for #{tz.name}" do
          first = g[1].joins(phase: :championship).where("championships.id in (?)", champs).where(has_time: true).order(:date).first.date
          last = g[1].joins(phase: :championship).where("championships.id in (?)", champs).where(has_time: true).order(:date).last.date
          while first < last
            g[1].joins(phase: :championship).where("championships.id in (?)", champs).where(has_time: true).order(:date).where("date > ? and date < ?", tz.period_for_local(first).start_transition.at.to_datetime, (tz.period_for_local(first).end_transition.try(:at).try(:to_datetime) or "2050-01-01".to_datetime)).update_all("date = TIMESTAMPADD(SECOND, -#{tz.period_for_local(first).offset.utc_total_offset}, date)")
            first = (tz.period_for_local(first).end_transition.try(:at).try(:to_datetime) or "2050-01-01".to_datetime) + 2.hours
          end
        end
      end
    end
  end
  def down
    [[:games, Game]].each do |g|
      TZ_CHAMP.each do |tz, champs|
        say_with_time "Updating games for #{tz.name}" do
          first = g[1].joins(phase: :championship).where("championships.id in (?)", champs).where(has_time: true).order(:date).first.date
          last = g[1].joins(phase: :championship).where("championships.id in (?)", champs).where(has_time: true).order(:date).last.date
          while first < last
            g[1].joins(phase: :championship).where("championships.id in (?)", champs).where(has_time: true).order(:date).where("date > ? and date < ?", tz.period_for_utc(first).start_transition.at.to_datetime, (tz.period_for_utc(first).end_transition.try(:at).try(:to_datetime) or "2050-01-01".to_datetime)).update_all("date = TIMESTAMPADD(SECOND, -#{tz.period_for_utc(first).offset.utc_total_offset}, date)")
            first = (tz.period_for_utc(first).end_transition.try(:at).try(:to_datetime) or "2050-01-01".to_datetime) + 2.hours
          end
        end
      end
    end
  end
end
