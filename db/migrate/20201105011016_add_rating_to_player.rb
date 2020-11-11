class AddRatingToPlayer < ActiveRecord::Migration
  def change
    add_column :players, :rating, :float
    add_column :players, :off_rating, :float
    add_column :players, :def_rating, :float
    add_index :players, :rating
  end
end
