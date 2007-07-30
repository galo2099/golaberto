class AddChampionshipCategory < ActiveRecord::Migration
  def self.up
    create_table "categories", :force => true do |t|
      t.column :name, :string
    end
    category = Category.new
    category.name = 'Profissional'
    category.save

    add_column :championships, :category_id, :integer, :limit => 20, :default => 1, :null => false
    change_column :championships, :category_id, :integer, :limit => 20, :default => 0, :null => false
  end

  def self.down
    remove_column :championships, :category_id
    drop_table :categories
  end
end
